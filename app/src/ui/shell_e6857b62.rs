#[allow(warnings, clippy::all)]
mod __ice_group_shell_e6857b62 {
use super::*;
impl super::Ducktape {
pub(super) fn __ice_component_use_63(&self, __ice_palette: __IcePalette, __ice_use_scope: ::std::string::String, __ice_cb_0: impl Fn() -> __DucktapeMessage + Clone + 'static) -> __IceElement<'_, __DucktapeMessage> { let __component_content: __IceElement<'_, __DucktapeMessage> = {
#[cfg(test)]
let __ice_render_source_location = ::ui_lang_runtime::testing::Location::new("components/shell.ice", 41, 1, "rendered view node");
#[cfg(test)]
let __ice_render_source = ::ui_lang_runtime::testing::push_render_source(__ice_render_source_location);
let __ice_rendered: __IceElement<'_, __DucktapeMessage> = { let __ice_node_scope = format!("{}/root", __ice_use_scope); { let __a11y_key = __ice_node_scope.clone(); let __a11y_id = ::ui_lang_runtime::StableId::new(&__a11y_key); let __disabled = false; let __activate = (__ice_cb_0)(); let __button_content: __IceElement<'_, __DucktapeMessage> = {
#[cfg(test)]
let __ice_render_source_location = ::ui_lang_runtime::testing::Location::new("components/shell.ice", 46, 1, "rendered view node");
#[cfg(test)]
let __ice_render_source = ::ui_lang_runtime::testing::push_render_source(__ice_render_source_location);
let __ice_rendered: __IceElement<'_, __DucktapeMessage> = { let mut __children: ::std::vec::Vec<__IceElement<'_, __DucktapeMessage>> = ::std::vec::Vec::new(); __children.push({
#[cfg(test)]
let __ice_render_source_location = ::ui_lang_runtime::testing::Location::new("components/shell.ice", 47, 1, "rendered view node");
#[cfg(test)]
let __ice_render_source = ::ui_lang_runtime::testing::push_render_source(__ice_render_source_location);
let __ice_rendered: __IceElement<'_, __DucktapeMessage> = { let __a11y_key = format!("{}/@container:47", __ice_use_scope); let __container_content: __IceElement<'_, __DucktapeMessage> = {
#[cfg(test)]
let __ice_render_source_location = ::ui_lang_runtime::testing::Location::new("components/shell.ice", 55, 1, "rendered view node");
#[cfg(test)]
let __ice_render_source = ::ui_lang_runtime::testing::push_render_source(__ice_render_source_location);
let __ice_rendered: __IceElement<'_, __DucktapeMessage> = { let __a11y_key = format!("{}/@text:55", __ice_use_scope); let __text_value = ("◆").to_string(); let __text = ::iced::widget::text(__text_value.clone()).size(((7.5) as f32).max(f32::EPSILON).min(f32::MAX)).wrapping(::iced::widget::text::Wrapping::None).font(::iced::Font { family: ::iced::font::Family::Name("Geist Mono"), weight: ::iced::font::Weight::Semibold, stretch: ::iced::font::Stretch::Normal, style: ::iced::font::Style::Normal }).color(__ice_palette.colors[38]); let __text = ::ui_lang_runtime::selectable_text(__text); ::ui_lang_runtime::accessible(__text, ::ui_lang_runtime::StableId::new(&__a11y_key), ::ui_lang_runtime::Role::Label).logical_id_maybe(::core::cfg!(test).then_some(__a11y_key)).value(__text_value).into() };
#[cfg(test)]
drop(__ice_render_source);
__ice_rendered
}; let __container = {  ::iced::widget::container(__container_content).width(15.0 as f32).height(15.0 as f32).align_x(::iced::alignment::Horizontal::Center).align_y(::iced::alignment::Vertical::Center).style(move |__theme| { let mut __style = ::iced::widget::container::Style::default(); __style.background = ::std::option::Option::Some(::iced::Background::Color(__ice_palette.colors[7])); __style.border.radius = ::iced::border::Radius { top_left: ((4.0) as f32).max(0.0).min(f32::MAX), top_right: ((4.0) as f32).max(0.0).min(f32::MAX), bottom_right: ((4.0) as f32).max(0.0).min(f32::MAX), bottom_left: ((4.0) as f32).max(0.0).min(f32::MAX) }; __style }) }; ::ui_lang_runtime::accessible(__container, ::ui_lang_runtime::StableId::new(&__a11y_key), ::ui_lang_runtime::Role::GenericContainer).logical_id_maybe(::core::cfg!(test).then_some(__a11y_key)).into() };
#[cfg(test)]
drop(__ice_render_source);
__ice_rendered
}); __children.push({
#[cfg(test)]
let __ice_render_source_location = ::ui_lang_runtime::testing::Location::new("components/shell.ice", 61, 1, "rendered view node");
#[cfg(test)]
let __ice_render_source = ::ui_lang_runtime::testing::push_render_source(__ice_render_source_location);
let __ice_rendered: __IceElement<'_, __DucktapeMessage> = { let __a11y_key = format!("{}/@text:61", __ice_use_scope); let __text_value = (self.network_name).to_string(); let __text = ::iced::widget::text(__text_value.clone()).size(((11.5) as f32).max(f32::EPSILON).min(f32::MAX)).wrapping(::iced::widget::text::Wrapping::None).font(::iced::Font { family: ::iced::font::Family::Name("Geist"), weight: ::iced::font::Weight::Semibold, stretch: ::iced::font::Stretch::Normal, style: ::iced::font::Style::Normal }).color(__ice_palette.colors[15]); let __text = ::ui_lang_runtime::selectable_text(__text); ::ui_lang_runtime::accessible(__text, ::ui_lang_runtime::StableId::new(&__a11y_key), ::ui_lang_runtime::Role::Label).logical_id_maybe(::core::cfg!(test).then_some(__a11y_key)).value(__text_value).into() };
#[cfg(test)]
drop(__ice_render_source);
__ice_rendered
}); let __child_count = __children.len(); let __children = __children.into_iter().map(|__child| ::ui_lang_runtime::bounded_fill_element(__child, __child_count, true)).collect::<::std::vec::Vec<_>>(); let __layout = ::iced::widget::row(__children).spacing(::ui_lang_runtime::bounded_spacing(7.0, __child_count)).align_y(::iced::alignment::Vertical::Center); let __content = ::iced::widget::container(__layout); let __layout_content: __IceElement<'_, __DucktapeMessage> = __content.into(); let __a11y_key = format!("{}/@layout:46", __ice_use_scope); ::ui_lang_runtime::accessible(__layout_content, ::ui_lang_runtime::StableId::new(&__a11y_key), ::ui_lang_runtime::Role::GenericContainer).logical_id_maybe(::core::cfg!(test).then_some(__a11y_key)).into() };
#[cfg(test)]
drop(__ice_render_source);
__ice_rendered
}; let __button = ::iced::widget::button(__button_content).padding(::iced::Padding { top: 7.0, right: 12.0, bottom: 7.0, left: 12.0 }).padding(4.0 as f32).on_press_maybe(if __disabled { None } else { Some(__activate.clone()) }).style(move |__theme, __status| { let mut __style = ::iced::widget::button::primary(__theme, __status); let __background: Option<::iced::Color> = match __status { ::iced::widget::button::Status::Hovered => Some(__ice_palette.colors[14]), ::iced::widget::button::Status::Pressed => Some(__ice_palette.colors[39]), ::iced::widget::button::Status::Disabled => Some(::iced::Color::from_rgba8(0, 0, 0, 0.000000)), _ => Some(::iced::Color::from_rgba8(0, 0, 0, 0.000000)) }; if let Some(__background) = __background { __style.background = Some(::iced::Background::Color(__background)); } __style.text_color = __ice_palette.colors[4]; __style.border.radius = 8.0.into(); __style.background = ::std::option::Option::Some(::iced::Background::Color(::iced::Color::from_rgba8(0, 0, 0, 0.000000))); __style.text_color = __ice_palette.colors[5]; __style.border.color = ::iced::Color::from_rgba8(0, 0, 0, 0.000000); __style.border.width = 1.0 as f32; __style.border.radius = ::iced::border::Radius { top_left: ((7.0) as f32).max(0.0).min(f32::MAX), top_right: ((7.0) as f32).max(0.0).min(f32::MAX), bottom_right: ((7.0) as f32).max(0.0).min(f32::MAX), bottom_left: ((7.0) as f32).max(0.0).min(f32::MAX) }; match __status { ::iced::widget::button::Status::Hovered => { __style.background = ::std::option::Option::Some(::iced::Background::Color(__ice_palette.colors[58])); __style.text_color = __ice_palette.colors[4]; } ::iced::widget::button::Status::Pressed => { __style.background = ::std::option::Option::Some(::iced::Background::Color(__ice_palette.colors[56])); __style.text_color = __ice_palette.colors[4]; } _ => {} } if matches!(__status, ::iced::widget::button::Status::Disabled) { if let Some(::iced::Background::Color(mut __color)) = __style.background { __color.a *= 0.5; __style.background = Some(::iced::Background::Color(__color)); } __style.text_color.a *= 0.5; } __style }); ::ui_lang_runtime::accessible(__button, __a11y_id, ::ui_lang_runtime::Role::Button).logical_id_maybe(::core::cfg!(test).then_some(&*__a11y_key)).focus_id(::iced::widget::Id::from(__a11y_key)).label("Switch network".to_owned()).disabled(__disabled).on_activate_maybe(if __disabled { None } else { Some(__activate) }).focus_ring(__ice_palette.colors[42], 8.0).into() } };
#[cfg(test)]
drop(__ice_render_source);
__ice_rendered
}; __component_content }
pub(super) fn __ice_component_use_64(&self, __ice_palette: __IcePalette, __ice_use_scope: ::std::string::String) -> __IceElement<'_, __DucktapeMessage> { let __component_content: __IceElement<'_, __DucktapeMessage> = {
#[cfg(test)]
let __ice_render_source_location = ::ui_lang_runtime::testing::Location::new("components/shell.ice", 75, 1, "rendered view node");
#[cfg(test)]
let __ice_render_source = ::ui_lang_runtime::testing::push_render_source(__ice_render_source_location);
let __ice_rendered: __IceElement<'_, __DucktapeMessage> = { let __ice_node_scope = format!("{}/root", __ice_use_scope); { let mut __children: ::std::vec::Vec<__IceElement<'_, __DucktapeMessage>> = ::std::vec::Vec::new(); if crate::backend::connection_degraded(::std::convert::AsRef::as_ref(&(self.status))) { __children.push({
#[cfg(test)]
let __ice_render_source_location = ::ui_lang_runtime::testing::Location::new("components/shell.ice", 77, 1, "rendered view node");
#[cfg(test)]
let __ice_render_source = ::ui_lang_runtime::testing::push_render_source(__ice_render_source_location);
let __ice_rendered: __IceElement<'_, __DucktapeMessage> = { let __a11y_key = format!("{}/@container:77", __ice_use_scope); let __container_content: __IceElement<'_, __DucktapeMessage> = {
#[cfg(test)]
let __ice_render_source_location = ::ui_lang_runtime::testing::Location::new("components/shell.ice", 83, 1, "rendered view node");
#[cfg(test)]
let __ice_render_source = ::ui_lang_runtime::testing::push_render_source(__ice_render_source_location);
let __ice_rendered: __IceElement<'_, __DucktapeMessage> = ::iced::widget::space().width(1.0 as f32).height(1.0 as f32).into();
#[cfg(test)]
drop(__ice_render_source);
__ice_rendered
}; let __container = {  ::iced::widget::container(__container_content).width(6.0 as f32).height(6.0 as f32).style(move |__theme| { let mut __style = ::iced::widget::container::Style::default(); __style.background = ::std::option::Option::Some(::iced::Background::Color(__ice_palette.colors[83])); __style.border.radius = ::iced::border::Radius { top_left: (((6.0 / 2.0)) as f32).max(0.0).min(f32::MAX), top_right: (((6.0 / 2.0)) as f32).max(0.0).min(f32::MAX), bottom_right: (((6.0 / 2.0)) as f32).max(0.0).min(f32::MAX), bottom_left: (((6.0 / 2.0)) as f32).max(0.0).min(f32::MAX) }; __style }) }; ::ui_lang_runtime::accessible(__container, ::ui_lang_runtime::StableId::new(&__a11y_key), ::ui_lang_runtime::Role::GenericContainer).logical_id_maybe(::core::cfg!(test).then_some(__a11y_key)).into() };
#[cfg(test)]
drop(__ice_render_source);
__ice_rendered
}); } if ((!crate::backend::connection_degraded(::std::convert::AsRef::as_ref(&(self.status)))) && (self.loading || (*self.__ice_derived_mutation_busy()))) { __children.push({
#[cfg(test)]
let __ice_render_source_location = ::ui_lang_runtime::testing::Location::new("components/shell.ice", 85, 1, "rendered view node");
#[cfg(test)]
let __ice_render_source = ::ui_lang_runtime::testing::push_render_source(__ice_render_source_location);
let __ice_rendered: __IceElement<'_, __DucktapeMessage> = { let __a11y_key = format!("{}/@container:85", __ice_use_scope); let __container_content: __IceElement<'_, __DucktapeMessage> = {
#[cfg(test)]
let __ice_render_source_location = ::ui_lang_runtime::testing::Location::new("components/shell.ice", 91, 1, "rendered view node");
#[cfg(test)]
let __ice_render_source = ::ui_lang_runtime::testing::push_render_source(__ice_render_source_location);
let __ice_rendered: __IceElement<'_, __DucktapeMessage> = ::iced::widget::space().width(1.0 as f32).height(1.0 as f32).into();
#[cfg(test)]
drop(__ice_render_source);
__ice_rendered
}; let __container = {  ::iced::widget::container(__container_content).width(6.0 as f32).height(6.0 as f32).style(move |__theme| { let mut __style = ::iced::widget::container::Style::default(); __style.background = ::std::option::Option::Some(::iced::Background::Color(__ice_palette.colors[34])); __style.border.radius = ::iced::border::Radius { top_left: (((6.0 / 2.0)) as f32).max(0.0).min(f32::MAX), top_right: (((6.0 / 2.0)) as f32).max(0.0).min(f32::MAX), bottom_right: (((6.0 / 2.0)) as f32).max(0.0).min(f32::MAX), bottom_left: (((6.0 / 2.0)) as f32).max(0.0).min(f32::MAX) }; __style }) }; ::ui_lang_runtime::accessible(__container, ::ui_lang_runtime::StableId::new(&__a11y_key), ::ui_lang_runtime::Role::GenericContainer).logical_id_maybe(::core::cfg!(test).then_some(__a11y_key)).into() };
#[cfg(test)]
drop(__ice_render_source);
__ice_rendered
}); } if ((!crate::backend::connection_degraded(::std::convert::AsRef::as_ref(&(self.status)))) && (!(self.loading || (*self.__ice_derived_mutation_busy())))) { __children.push({
#[cfg(test)]
let __ice_render_source_location = ::ui_lang_runtime::testing::Location::new("components/shell.ice", 93, 1, "rendered view node");
#[cfg(test)]
let __ice_render_source = ::ui_lang_runtime::testing::push_render_source(__ice_render_source_location);
let __ice_rendered: __IceElement<'_, __DucktapeMessage> = { let __a11y_key = format!("{}/@container:93", __ice_use_scope); let __container_content: __IceElement<'_, __DucktapeMessage> = {
#[cfg(test)]
let __ice_render_source_location = ::ui_lang_runtime::testing::Location::new("components/shell.ice", 99, 1, "rendered view node");
#[cfg(test)]
let __ice_render_source = ::ui_lang_runtime::testing::push_render_source(__ice_render_source_location);
let __ice_rendered: __IceElement<'_, __DucktapeMessage> = ::iced::widget::space().width(1.0 as f32).height(1.0 as f32).into();
#[cfg(test)]
drop(__ice_render_source);
__ice_rendered
}; let __container = {  ::iced::widget::container(__container_content).width(6.0 as f32).height(6.0 as f32).style(move |__theme| { let mut __style = ::iced::widget::container::Style::default(); __style.background = ::std::option::Option::Some(::iced::Background::Color(__ice_palette.colors[29])); __style.border.radius = ::iced::border::Radius { top_left: (((6.0 / 2.0)) as f32).max(0.0).min(f32::MAX), top_right: (((6.0 / 2.0)) as f32).max(0.0).min(f32::MAX), bottom_right: (((6.0 / 2.0)) as f32).max(0.0).min(f32::MAX), bottom_left: (((6.0 / 2.0)) as f32).max(0.0).min(f32::MAX) }; __style }) }; ::ui_lang_runtime::accessible(__container, ::ui_lang_runtime::StableId::new(&__a11y_key), ::ui_lang_runtime::Role::GenericContainer).logical_id_maybe(::core::cfg!(test).then_some(__a11y_key)).into() };
#[cfg(test)]
drop(__ice_render_source);
__ice_rendered
}); } let __child_count = __children.len(); let __children = __children.into_iter().map(|__child| ::ui_lang_runtime::bounded_fill_element(__child, __child_count, false)).collect::<::std::vec::Vec<_>>(); let __layout = ::iced::widget::column(__children); let __content = ::iced::widget::container(__layout); let __layout_content: __IceElement<'_, __DucktapeMessage> = __content.into(); let __a11y_key = __ice_node_scope.as_str(); ::ui_lang_runtime::accessible(__layout_content, ::ui_lang_runtime::StableId::new(&__a11y_key), ::ui_lang_runtime::Role::GenericContainer).logical_id_maybe(::core::cfg!(test).then_some(__a11y_key)).into() } };
#[cfg(test)]
drop(__ice_render_source);
__ice_rendered
}; __component_content }
pub(super) fn __ice_component_use_65(&self, __ice_palette: __IcePalette, __ice_use_scope: ::std::string::String) -> __IceElement<'_, __DucktapeMessage> { let __component_content: __IceElement<'_, __DucktapeMessage> = {
#[cfg(test)]
let __ice_render_source_location = ::ui_lang_runtime::testing::Location::new("components/shell.ice", 104, 1, "rendered view node");
#[cfg(test)]
let __ice_render_source = ::ui_lang_runtime::testing::push_render_source(__ice_render_source_location);
let __ice_rendered: __IceElement<'_, __DucktapeMessage> = { let __ice_node_scope = format!("{}/root", __ice_use_scope); { let __a11y_key = __ice_node_scope.as_str(); let __container_content: __IceElement<'_, __DucktapeMessage> = {
#[cfg(test)]
let __ice_render_source_location = ::ui_lang_runtime::testing::Location::new("components/shell.ice", 112, 1, "rendered view node");
#[cfg(test)]
let __ice_render_source = ::ui_lang_runtime::testing::push_render_source(__ice_render_source_location);
let __ice_rendered: __IceElement<'_, __DucktapeMessage> = { let mut __children: ::std::vec::Vec<__IceElement<'_, __DucktapeMessage>> = ::std::vec::Vec::new(); __children.push({
#[cfg(test)]
let __ice_render_source_location = ::ui_lang_runtime::testing::Location::new("components/shell.ice", 113, 1, "rendered view node");
#[cfg(test)]
let __ice_render_source = ::ui_lang_runtime::testing::push_render_source(__ice_render_source_location);
let __ice_rendered: __IceElement<'_, __DucktapeMessage> = { let __ice_component_0537461747573446f74_scope_3033 = format!("{}/StatusDot@3033", __ice_use_scope); ::ui_lang_runtime::rev_memo(313u64, (self.__ice_rev[19], self.__ice_rev[7], self.__ice_rev[9], __ice_palette.name), ::ui_lang_runtime::grow_stack(|| self.__ice_component_use_64(__ice_palette, __ice_component_0537461747573446f74_scope_3033))).into() };
#[cfg(test)]
drop(__ice_render_source);
__ice_rendered
}); if crate::backend::connection_degraded(::std::convert::AsRef::as_ref(&(self.status))) { __children.push({
#[cfg(test)]
let __ice_render_source_location = ::ui_lang_runtime::testing::Location::new("components/shell.ice", 119, 1, "rendered view node");
#[cfg(test)]
let __ice_render_source = ::ui_lang_runtime::testing::push_render_source(__ice_render_source_location);
let __ice_rendered: __IceElement<'_, __DucktapeMessage> = { let __a11y_key = format!("{}/@text:119", __ice_use_scope); let __text_value = ("Stopped").to_string(); let __text = ::iced::widget::text(__text_value.clone()).size(((10.5) as f32).max(f32::EPSILON).min(f32::MAX)).wrapping(::iced::widget::text::Wrapping::None).font(::iced::Font { family: ::iced::font::Family::Name("Geist Mono"), weight: ::iced::font::Weight::Medium, stretch: ::iced::font::Stretch::Normal, style: ::iced::font::Style::Normal }).color(__ice_palette.colors[41]); let __text = ::ui_lang_runtime::selectable_text(__text); ::ui_lang_runtime::accessible(__text, ::ui_lang_runtime::StableId::new(&__a11y_key), ::ui_lang_runtime::Role::Label).logical_id_maybe(::core::cfg!(test).then_some(__a11y_key)).value(__text_value).into() };
#[cfg(test)]
drop(__ice_render_source);
__ice_rendered
}); } if ((!crate::backend::connection_degraded(::std::convert::AsRef::as_ref(&(self.status)))) && (self.loading || (*self.__ice_derived_mutation_busy()))) { __children.push({
#[cfg(test)]
let __ice_render_source_location = ::ui_lang_runtime::testing::Location::new("components/shell.ice", 126, 1, "rendered view node");
#[cfg(test)]
let __ice_render_source = ::ui_lang_runtime::testing::push_render_source(__ice_render_source_location);
let __ice_rendered: __IceElement<'_, __DucktapeMessage> = { let __a11y_key = format!("{}/@text:126", __ice_use_scope); let __text_value = ("Syncing…").to_string(); let __text = ::iced::widget::text(__text_value.clone()).size(((10.5) as f32).max(f32::EPSILON).min(f32::MAX)).wrapping(::iced::widget::text::Wrapping::None).font(::iced::Font { family: ::iced::font::Family::Name("Geist Mono"), weight: ::iced::font::Weight::Medium, stretch: ::iced::font::Stretch::Normal, style: ::iced::font::Style::Normal }).color(__ice_palette.colors[41]); let __text = ::ui_lang_runtime::selectable_text(__text); ::ui_lang_runtime::accessible(__text, ::ui_lang_runtime::StableId::new(&__a11y_key), ::ui_lang_runtime::Role::Label).logical_id_maybe(::core::cfg!(test).then_some(__a11y_key)).value(__text_value).into() };
#[cfg(test)]
drop(__ice_render_source);
__ice_rendered
}); } if ((!crate::backend::connection_degraded(::std::convert::AsRef::as_ref(&(self.status)))) && (!(self.loading || (*self.__ice_derived_mutation_busy())))) { __children.push({
#[cfg(test)]
let __ice_render_source_location = ::ui_lang_runtime::testing::Location::new("components/shell.ice", 133, 1, "rendered view node");
#[cfg(test)]
let __ice_render_source = ::ui_lang_runtime::testing::push_render_source(__ice_render_source_location);
let __ice_rendered: __IceElement<'_, __DucktapeMessage> = { let __a11y_key = format!("{}/@text:133", __ice_use_scope); let __text_value = ("Synced").to_string(); let __text = ::iced::widget::text(__text_value.clone()).size(((10.5) as f32).max(f32::EPSILON).min(f32::MAX)).wrapping(::iced::widget::text::Wrapping::None).font(::iced::Font { family: ::iced::font::Family::Name("Geist Mono"), weight: ::iced::font::Weight::Medium, stretch: ::iced::font::Stretch::Normal, style: ::iced::font::Style::Normal }).color(__ice_palette.colors[41]); let __text = ::ui_lang_runtime::selectable_text(__text); ::ui_lang_runtime::accessible(__text, ::ui_lang_runtime::StableId::new(&__a11y_key), ::ui_lang_runtime::Role::Label).logical_id_maybe(::core::cfg!(test).then_some(__a11y_key)).value(__text_value).into() };
#[cfg(test)]
drop(__ice_render_source);
__ice_rendered
}); } let __child_count = __children.len(); let __children = __children.into_iter().map(|__child| ::ui_lang_runtime::bounded_fill_element(__child, __child_count, true)).collect::<::std::vec::Vec<_>>(); let __layout = ::iced::widget::row(__children).spacing(::ui_lang_runtime::bounded_spacing(5.0, __child_count)).align_y(::iced::alignment::Vertical::Center); let __content = ::iced::widget::container(__layout); let __layout_content: __IceElement<'_, __DucktapeMessage> = __content.into(); let __a11y_key = format!("{}/@layout:112", __ice_use_scope); ::ui_lang_runtime::accessible(__layout_content, ::ui_lang_runtime::StableId::new(&__a11y_key), ::ui_lang_runtime::Role::GenericContainer).logical_id_maybe(::core::cfg!(test).then_some(__a11y_key)).into() };
#[cfg(test)]
drop(__ice_render_source);
__ice_rendered
}; let __container = {  ::iced::widget::container(__container_content).id(::iced::widget::Id::from(__ice_node_scope.clone())).padding(::ui_lang_runtime::bounded_padding(3.0, 8.0, 3.0, 8.0)).style(move |__theme| { let mut __style = ::iced::widget::container::Style::default(); __style.background = ::std::option::Option::Some(::iced::Background::Color(__ice_palette.colors[3])); __style.border.color = __ice_palette.colors[39]; __style.border.width = 1.0 as f32; __style.border.radius = ::iced::border::Radius { top_left: ((7.0) as f32).max(0.0).min(f32::MAX), top_right: ((7.0) as f32).max(0.0).min(f32::MAX), bottom_right: ((7.0) as f32).max(0.0).min(f32::MAX), bottom_left: ((7.0) as f32).max(0.0).min(f32::MAX) }; __style }) }; ::ui_lang_runtime::accessible(__container, ::ui_lang_runtime::StableId::new(&__a11y_key), ::ui_lang_runtime::Role::GenericContainer).logical_id_maybe(::core::cfg!(test).then_some(__a11y_key)).into() } };
#[cfg(test)]
drop(__ice_render_source);
__ice_rendered
}; __component_content }
pub(super) fn __ice_component_use_66(&self, __ice_palette: __IcePalette, __ice_use_scope: ::std::string::String) -> __IceElement<'_, __DucktapeMessage> { let __component_content: __IceElement<'_, __DucktapeMessage> = {
#[cfg(test)]
let __ice_render_source_location = ::ui_lang_runtime::testing::Location::new("components/shell.ice", 75, 1, "rendered view node");
#[cfg(test)]
let __ice_render_source = ::ui_lang_runtime::testing::push_render_source(__ice_render_source_location);
let __ice_rendered: __IceElement<'_, __DucktapeMessage> = { let __ice_node_scope = format!("{}/root", __ice_use_scope); { let mut __children: ::std::vec::Vec<__IceElement<'_, __DucktapeMessage>> = ::std::vec::Vec::new(); if crate::backend::connection_degraded(::std::convert::AsRef::as_ref(&(self.status))) { __children.push({
#[cfg(test)]
let __ice_render_source_location = ::ui_lang_runtime::testing::Location::new("components/shell.ice", 77, 1, "rendered view node");
#[cfg(test)]
let __ice_render_source = ::ui_lang_runtime::testing::push_render_source(__ice_render_source_location);
let __ice_rendered: __IceElement<'_, __DucktapeMessage> = { let __a11y_key = format!("{}/@container:77", __ice_use_scope); let __container_content: __IceElement<'_, __DucktapeMessage> = {
#[cfg(test)]
let __ice_render_source_location = ::ui_lang_runtime::testing::Location::new("components/shell.ice", 83, 1, "rendered view node");
#[cfg(test)]
let __ice_render_source = ::ui_lang_runtime::testing::push_render_source(__ice_render_source_location);
let __ice_rendered: __IceElement<'_, __DucktapeMessage> = ::iced::widget::space().width(1.0 as f32).height(1.0 as f32).into();
#[cfg(test)]
drop(__ice_render_source);
__ice_rendered
}; let __container = {  ::iced::widget::container(__container_content).width(7.0 as f32).height(7.0 as f32).style(move |__theme| { let mut __style = ::iced::widget::container::Style::default(); __style.background = ::std::option::Option::Some(::iced::Background::Color(__ice_palette.colors[83])); __style.border.radius = ::iced::border::Radius { top_left: (((7.0 / 2.0)) as f32).max(0.0).min(f32::MAX), top_right: (((7.0 / 2.0)) as f32).max(0.0).min(f32::MAX), bottom_right: (((7.0 / 2.0)) as f32).max(0.0).min(f32::MAX), bottom_left: (((7.0 / 2.0)) as f32).max(0.0).min(f32::MAX) }; __style }) }; ::ui_lang_runtime::accessible(__container, ::ui_lang_runtime::StableId::new(&__a11y_key), ::ui_lang_runtime::Role::GenericContainer).logical_id_maybe(::core::cfg!(test).then_some(__a11y_key)).into() };
#[cfg(test)]
drop(__ice_render_source);
__ice_rendered
}); } if ((!crate::backend::connection_degraded(::std::convert::AsRef::as_ref(&(self.status)))) && (self.loading || (*self.__ice_derived_mutation_busy()))) { __children.push({
#[cfg(test)]
let __ice_render_source_location = ::ui_lang_runtime::testing::Location::new("components/shell.ice", 85, 1, "rendered view node");
#[cfg(test)]
let __ice_render_source = ::ui_lang_runtime::testing::push_render_source(__ice_render_source_location);
let __ice_rendered: __IceElement<'_, __DucktapeMessage> = { let __a11y_key = format!("{}/@container:85", __ice_use_scope); let __container_content: __IceElement<'_, __DucktapeMessage> = {
#[cfg(test)]
let __ice_render_source_location = ::ui_lang_runtime::testing::Location::new("components/shell.ice", 91, 1, "rendered view node");
#[cfg(test)]
let __ice_render_source = ::ui_lang_runtime::testing::push_render_source(__ice_render_source_location);
let __ice_rendered: __IceElement<'_, __DucktapeMessage> = ::iced::widget::space().width(1.0 as f32).height(1.0 as f32).into();
#[cfg(test)]
drop(__ice_render_source);
__ice_rendered
}; let __container = {  ::iced::widget::container(__container_content).width(7.0 as f32).height(7.0 as f32).style(move |__theme| { let mut __style = ::iced::widget::container::Style::default(); __style.background = ::std::option::Option::Some(::iced::Background::Color(__ice_palette.colors[34])); __style.border.radius = ::iced::border::Radius { top_left: (((7.0 / 2.0)) as f32).max(0.0).min(f32::MAX), top_right: (((7.0 / 2.0)) as f32).max(0.0).min(f32::MAX), bottom_right: (((7.0 / 2.0)) as f32).max(0.0).min(f32::MAX), bottom_left: (((7.0 / 2.0)) as f32).max(0.0).min(f32::MAX) }; __style }) }; ::ui_lang_runtime::accessible(__container, ::ui_lang_runtime::StableId::new(&__a11y_key), ::ui_lang_runtime::Role::GenericContainer).logical_id_maybe(::core::cfg!(test).then_some(__a11y_key)).into() };
#[cfg(test)]
drop(__ice_render_source);
__ice_rendered
}); } if ((!crate::backend::connection_degraded(::std::convert::AsRef::as_ref(&(self.status)))) && (!(self.loading || (*self.__ice_derived_mutation_busy())))) { __children.push({
#[cfg(test)]
let __ice_render_source_location = ::ui_lang_runtime::testing::Location::new("components/shell.ice", 93, 1, "rendered view node");
#[cfg(test)]
let __ice_render_source = ::ui_lang_runtime::testing::push_render_source(__ice_render_source_location);
let __ice_rendered: __IceElement<'_, __DucktapeMessage> = { let __a11y_key = format!("{}/@container:93", __ice_use_scope); let __container_content: __IceElement<'_, __DucktapeMessage> = {
#[cfg(test)]
let __ice_render_source_location = ::ui_lang_runtime::testing::Location::new("components/shell.ice", 99, 1, "rendered view node");
#[cfg(test)]
let __ice_render_source = ::ui_lang_runtime::testing::push_render_source(__ice_render_source_location);
let __ice_rendered: __IceElement<'_, __DucktapeMessage> = ::iced::widget::space().width(1.0 as f32).height(1.0 as f32).into();
#[cfg(test)]
drop(__ice_render_source);
__ice_rendered
}; let __container = {  ::iced::widget::container(__container_content).width(7.0 as f32).height(7.0 as f32).style(move |__theme| { let mut __style = ::iced::widget::container::Style::default(); __style.background = ::std::option::Option::Some(::iced::Background::Color(__ice_palette.colors[29])); __style.border.radius = ::iced::border::Radius { top_left: (((7.0 / 2.0)) as f32).max(0.0).min(f32::MAX), top_right: (((7.0 / 2.0)) as f32).max(0.0).min(f32::MAX), bottom_right: (((7.0 / 2.0)) as f32).max(0.0).min(f32::MAX), bottom_left: (((7.0 / 2.0)) as f32).max(0.0).min(f32::MAX) }; __style }) }; ::ui_lang_runtime::accessible(__container, ::ui_lang_runtime::StableId::new(&__a11y_key), ::ui_lang_runtime::Role::GenericContainer).logical_id_maybe(::core::cfg!(test).then_some(__a11y_key)).into() };
#[cfg(test)]
drop(__ice_render_source);
__ice_rendered
}); } let __child_count = __children.len(); let __children = __children.into_iter().map(|__child| ::ui_lang_runtime::bounded_fill_element(__child, __child_count, false)).collect::<::std::vec::Vec<_>>(); let __layout = ::iced::widget::column(__children); let __content = ::iced::widget::container(__layout); let __layout_content: __IceElement<'_, __DucktapeMessage> = __content.into(); let __a11y_key = __ice_node_scope.as_str(); ::ui_lang_runtime::accessible(__layout_content, ::ui_lang_runtime::StableId::new(&__a11y_key), ::ui_lang_runtime::Role::GenericContainer).logical_id_maybe(::core::cfg!(test).then_some(__a11y_key)).into() } };
#[cfg(test)]
drop(__ice_render_source);
__ice_rendered
}; __component_content }
pub(super) fn __ice_component_use_67(&self, __ice_palette: __IcePalette, __ice_use_scope: ::std::string::String) -> __IceElement<'_, __DucktapeMessage> { let __component_content: __IceElement<'_, __DucktapeMessage> = {
#[cfg(test)]
let __ice_render_source_location = ::ui_lang_runtime::testing::Location::new("components/shell.ice", 166, 1, "rendered view node");
#[cfg(test)]
let __ice_render_source = ::ui_lang_runtime::testing::push_render_source(__ice_render_source_location);
let __ice_rendered: __IceElement<'_, __DucktapeMessage> = { let __ice_node_scope = format!("{}/root", __ice_use_scope); { let __a11y_key = __ice_node_scope.as_str(); let __container_content: __IceElement<'_, __DucktapeMessage> = {
#[cfg(test)]
let __ice_render_source_location = ::ui_lang_runtime::testing::Location::new("components/shell.ice", 180, 1, "rendered view node");
#[cfg(test)]
let __ice_render_source = ::ui_lang_runtime::testing::push_render_source(__ice_render_source_location);
let __ice_rendered: __IceElement<'_, __DucktapeMessage> = { let mut __children: ::std::vec::Vec<__IceElement<'_, __DucktapeMessage>> = ::std::vec::Vec::new(); __children.push({
#[cfg(test)]
let __ice_render_source_location = ::ui_lang_runtime::testing::Location::new("components/shell.ice", 181, 1, "rendered view node");
#[cfg(test)]
let __ice_render_source = ::ui_lang_runtime::testing::push_render_source(__ice_render_source_location);
let __ice_rendered: __IceElement<'_, __DucktapeMessage> = { let mut __children: ::std::vec::Vec<__IceElement<'_, __DucktapeMessage>> = ::std::vec::Vec::new(); __children.push({
#[cfg(test)]
let __ice_render_source_location = ::ui_lang_runtime::testing::Location::new("components/shell.ice", 186, 1, "rendered view node");
#[cfg(test)]
let __ice_render_source = ::ui_lang_runtime::testing::push_render_source(__ice_render_source_location);
let __ice_rendered: __IceElement<'_, __DucktapeMessage> = { let __ice_component_0537461747573446f74_scope_3106 = format!("{}/StatusDot@3106", __ice_use_scope); ::ui_lang_runtime::rev_memo(323u64, (self.__ice_rev[19], self.__ice_rev[7], self.__ice_rev[9], __ice_palette.name), ::ui_lang_runtime::grow_stack(|| self.__ice_component_use_66(__ice_palette, __ice_component_0537461747573446f74_scope_3106))).into() };
#[cfg(test)]
drop(__ice_render_source);
__ice_rendered
}); if crate::backend::connection_degraded(::std::convert::AsRef::as_ref(&(self.status))) { __children.push({
#[cfg(test)]
let __ice_render_source_location = ::ui_lang_runtime::testing::Location::new("components/shell.ice", 192, 1, "rendered view node");
#[cfg(test)]
let __ice_render_source = ::ui_lang_runtime::testing::push_render_source(__ice_render_source_location);
let __ice_rendered: __IceElement<'_, __DucktapeMessage> = { let __a11y_key = format!("{}/@text:192", __ice_use_scope); let __text_value = ("Stopped").to_string(); let __text = ::iced::widget::text(__text_value.clone()).size(((12.0) as f32).max(f32::EPSILON).min(f32::MAX)).wrapping(::iced::widget::text::Wrapping::None).font(::iced::Font { family: ::iced::font::Family::Name("Geist"), weight: ::iced::font::Weight::Semibold, stretch: ::iced::font::Stretch::Normal, style: ::iced::font::Style::Normal }).color(__ice_palette.colors[7]); let __text = ::ui_lang_runtime::selectable_text(__text); ::ui_lang_runtime::accessible(__text, ::ui_lang_runtime::StableId::new(&__a11y_key), ::ui_lang_runtime::Role::Label).logical_id_maybe(::core::cfg!(test).then_some(__a11y_key)).value(__text_value).into() };
#[cfg(test)]
drop(__ice_render_source);
__ice_rendered
}); } if ((!crate::backend::connection_degraded(::std::convert::AsRef::as_ref(&(self.status)))) && (self.loading || (*self.__ice_derived_mutation_busy()))) { __children.push({
#[cfg(test)]
let __ice_render_source_location = ::ui_lang_runtime::testing::Location::new("components/shell.ice", 199, 1, "rendered view node");
#[cfg(test)]
let __ice_render_source = ::ui_lang_runtime::testing::push_render_source(__ice_render_source_location);
let __ice_rendered: __IceElement<'_, __DucktapeMessage> = { let __a11y_key = format!("{}/@text:199", __ice_use_scope); let __text_value = ("Syncing…").to_string(); let __text = ::iced::widget::text(__text_value.clone()).size(((12.0) as f32).max(f32::EPSILON).min(f32::MAX)).wrapping(::iced::widget::text::Wrapping::None).font(::iced::Font { family: ::iced::font::Family::Name("Geist"), weight: ::iced::font::Weight::Semibold, stretch: ::iced::font::Stretch::Normal, style: ::iced::font::Style::Normal }).color(__ice_palette.colors[7]); let __text = ::ui_lang_runtime::selectable_text(__text); ::ui_lang_runtime::accessible(__text, ::ui_lang_runtime::StableId::new(&__a11y_key), ::ui_lang_runtime::Role::Label).logical_id_maybe(::core::cfg!(test).then_some(__a11y_key)).value(__text_value).into() };
#[cfg(test)]
drop(__ice_render_source);
__ice_rendered
}); } if ((!crate::backend::connection_degraded(::std::convert::AsRef::as_ref(&(self.status)))) && (!(self.loading || (*self.__ice_derived_mutation_busy())))) { __children.push({
#[cfg(test)]
let __ice_render_source_location = ::ui_lang_runtime::testing::Location::new("components/shell.ice", 206, 1, "rendered view node");
#[cfg(test)]
let __ice_render_source = ::ui_lang_runtime::testing::push_render_source(__ice_render_source_location);
let __ice_rendered: __IceElement<'_, __DucktapeMessage> = { let __a11y_key = format!("{}/@text:206", __ice_use_scope); let __text_value = ("Synced").to_string(); let __text = ::iced::widget::text(__text_value.clone()).size(((12.0) as f32).max(f32::EPSILON).min(f32::MAX)).wrapping(::iced::widget::text::Wrapping::None).font(::iced::Font { family: ::iced::font::Family::Name("Geist"), weight: ::iced::font::Weight::Semibold, stretch: ::iced::font::Stretch::Normal, style: ::iced::font::Style::Normal }).color(__ice_palette.colors[7]); let __text = ::ui_lang_runtime::selectable_text(__text); ::ui_lang_runtime::accessible(__text, ::ui_lang_runtime::StableId::new(&__a11y_key), ::ui_lang_runtime::Role::Label).logical_id_maybe(::core::cfg!(test).then_some(__a11y_key)).value(__text_value).into() };
#[cfg(test)]
drop(__ice_render_source);
__ice_rendered
}); } __children.push({
#[cfg(test)]
let __ice_render_source_location = ::ui_lang_runtime::testing::Location::new("components/shell.ice", 212, 1, "rendered view node");
#[cfg(test)]
let __ice_render_source = ::ui_lang_runtime::testing::push_render_source(__ice_render_source_location);
let __ice_rendered: __IceElement<'_, __DucktapeMessage> = ::iced::widget::space().width(::iced::Fill).into();
#[cfg(test)]
drop(__ice_render_source);
__ice_rendered
}); if (!(crate::backend::member_tier(::std::convert::AsRef::as_ref(&(self.members_rows)))).is_empty()) { __children.push({
#[cfg(test)]
let __ice_render_source_location = ::ui_lang_runtime::testing::Location::new("components/shell.ice", 222, 1, "rendered view node");
#[cfg(test)]
let __ice_render_source = ::ui_lang_runtime::testing::push_render_source(__ice_render_source_location);
let __ice_rendered: __IceElement<'_, __DucktapeMessage> = { let __a11y_key = format!("{}/@text:222", __ice_use_scope); let __text_value = (crate::backend::member_tier(::std::convert::AsRef::as_ref(&(self.members_rows)))).to_string(); let __text = ::iced::widget::text(__text_value.clone()).size(((10.5) as f32).max(f32::EPSILON).min(f32::MAX)).wrapping(::iced::widget::text::Wrapping::None).font(::iced::Font { family: ::iced::font::Family::Name("Geist Mono"), weight: ::iced::font::Weight::Medium, stretch: ::iced::font::Stretch::Normal, style: ::iced::font::Style::Normal }).color(__ice_palette.colors[71]); let __text = ::ui_lang_runtime::selectable_text(__text); ::ui_lang_runtime::accessible(__text, ::ui_lang_runtime::StableId::new(&__a11y_key), ::ui_lang_runtime::Role::Label).logical_id_maybe(::core::cfg!(test).then_some(__a11y_key)).value(__text_value).into() };
#[cfg(test)]
drop(__ice_render_source);
__ice_rendered
}); } if ((crate::backend::member_tier(::std::convert::AsRef::as_ref(&(self.members_rows)))).is_empty() && self.members_answered) { __children.push({
#[cfg(test)]
let __ice_render_source_location = ::ui_lang_runtime::testing::Location::new("components/shell.ice", 229, 1, "rendered view node");
#[cfg(test)]
let __ice_render_source = ::ui_lang_runtime::testing::push_render_source(__ice_render_source_location);
let __ice_rendered: __IceElement<'_, __DucktapeMessage> = { let __a11y_key = format!("{}/@text:229", __ice_use_scope); let __text_value = ("standing unknown").to_string(); let __text = ::iced::widget::text(__text_value.clone()).size(((10.5) as f32).max(f32::EPSILON).min(f32::MAX)).wrapping(::iced::widget::text::Wrapping::None).font(::iced::Font { family: ::iced::font::Family::Name("Geist Mono"), weight: ::iced::font::Weight::Medium, stretch: ::iced::font::Stretch::Normal, style: ::iced::font::Style::Normal }).color(__ice_palette.colors[71]); let __text = ::ui_lang_runtime::selectable_text(__text); ::ui_lang_runtime::accessible(__text, ::ui_lang_runtime::StableId::new(&__a11y_key), ::ui_lang_runtime::Role::Label).logical_id_maybe(::core::cfg!(test).then_some(__a11y_key)).value(__text_value).into() };
#[cfg(test)]
drop(__ice_render_source);
__ice_rendered
}); } let __child_count = __children.len(); let __children = __children.into_iter().map(|__child| ::ui_lang_runtime::bounded_fill_element(__child, __child_count, true)).collect::<::std::vec::Vec<_>>(); let __layout = ::iced::widget::row(__children).spacing(::ui_lang_runtime::bounded_spacing(7.0, __child_count)).width(::iced::Fill).align_y(::iced::alignment::Vertical::Center); let __content = ::iced::widget::container(__layout); let __layout_content: __IceElement<'_, __DucktapeMessage> = __content.into(); let __a11y_key = format!("{}/@layout:181", __ice_use_scope); ::ui_lang_runtime::accessible(__layout_content, ::ui_lang_runtime::StableId::new(&__a11y_key), ::ui_lang_runtime::Role::GenericContainer).logical_id_maybe(::core::cfg!(test).then_some(__a11y_key)).into() };
#[cfg(test)]
drop(__ice_render_source);
__ice_rendered
}); __children.push({
#[cfg(test)]
let __ice_render_source_location = ::ui_lang_runtime::testing::Location::new("components/shell.ice", 235, 1, "rendered view node");
#[cfg(test)]
let __ice_render_source = ::ui_lang_runtime::testing::push_render_source(__ice_render_source_location);
let __ice_rendered: __IceElement<'_, __DucktapeMessage> = { let __a11y_key = format!("{}/@text:235", __ice_use_scope); let __text_value = (crate::backend::height_label(self.block_height)).to_string(); let __text = ::iced::widget::text(__text_value.clone()).size(((14.0) as f32).max(f32::EPSILON).min(f32::MAX)).wrapping(::iced::widget::text::Wrapping::None).font(::iced::Font { family: ::iced::font::Family::Name("Geist Mono"), weight: ::iced::font::Weight::Semibold, stretch: ::iced::font::Stretch::Normal, style: ::iced::font::Style::Normal }).color(__ice_palette.colors[7]); let __text = ::ui_lang_runtime::selectable_text(__text); ::ui_lang_runtime::accessible(__text, ::ui_lang_runtime::StableId::new(&__a11y_key), ::ui_lang_runtime::Role::Label).logical_id_maybe(::core::cfg!(test).then_some(__a11y_key)).value(__text_value).into() };
#[cfg(test)]
drop(__ice_render_source);
__ice_rendered
}); if (!(crate::backend::sync_label(::std::convert::AsRef::as_ref(&(self.node_phase)), self.node_sync_applied, self.node_sync_target)).is_empty()) { __children.push({
#[cfg(test)]
let __ice_render_source_location = ::ui_lang_runtime::testing::Location::new("components/shell.ice", 246, 1, "rendered view node");
#[cfg(test)]
let __ice_render_source = ::ui_lang_runtime::testing::push_render_source(__ice_render_source_location);
let __ice_rendered: __IceElement<'_, __DucktapeMessage> = { let __a11y_key = format!("{}/@text:246", __ice_use_scope); let __text_value = (crate::backend::sync_label(::std::convert::AsRef::as_ref(&(self.node_phase)), self.node_sync_applied, self.node_sync_target)).to_string(); let __text = ::iced::widget::text(__text_value.clone()).size(((11.0) as f32).max(f32::EPSILON).min(f32::MAX)).wrapping(::iced::widget::text::Wrapping::None).color(__ice_palette.colors[71]); let __text = ::ui_lang_runtime::selectable_text(__text); ::ui_lang_runtime::accessible(__text, ::ui_lang_runtime::StableId::new(&__a11y_key), ::ui_lang_runtime::Role::Label).logical_id_maybe(::core::cfg!(test).then_some(__a11y_key)).value(__text_value).into() };
#[cfg(test)]
drop(__ice_render_source);
__ice_rendered
}); } __children.push({
#[cfg(test)]
let __ice_render_source_location = ::ui_lang_runtime::testing::Location::new("components/shell.ice", 251, 1, "rendered view node");
#[cfg(test)]
let __ice_render_source = ::ui_lang_runtime::testing::push_render_source(__ice_render_source_location);
let __ice_rendered: __IceElement<'_, __DucktapeMessage> = { let mut __children: ::std::vec::Vec<__IceElement<'_, __DucktapeMessage>> = ::std::vec::Vec::new(); __children.push({
#[cfg(test)]
let __ice_render_source_location = ::ui_lang_runtime::testing::Location::new("components/shell.ice", 252, 1, "rendered view node");
#[cfg(test)]
let __ice_render_source = ::ui_lang_runtime::testing::push_render_source(__ice_render_source_location);
let __ice_rendered: __IceElement<'_, __DucktapeMessage> = { let mut __children: ::std::vec::Vec<__IceElement<'_, __DucktapeMessage>> = ::std::vec::Vec::new(); __children.push({
#[cfg(test)]
let __ice_render_source_location = ::ui_lang_runtime::testing::Location::new("components/shell.ice", 257, 1, "rendered view node");
#[cfg(test)]
let __ice_render_source = ::ui_lang_runtime::testing::push_render_source(__ice_render_source_location);
let __ice_rendered: __IceElement<'_, __DucktapeMessage> = { let __a11y_key = format!("{}/@text:257", __ice_use_scope); let __text_value = ("app-hash").to_string(); let __text = ::iced::widget::text(__text_value.clone()).size(((10.5) as f32).max(f32::EPSILON).min(f32::MAX)).wrapping(::iced::widget::text::Wrapping::None).font(::iced::Font { family: ::iced::font::Family::Name("Geist Mono"), weight: ::iced::font::Weight::Medium, stretch: ::iced::font::Stretch::Normal, style: ::iced::font::Style::Normal }).color(__ice_palette.colors[72]); let __text = ::ui_lang_runtime::selectable_text(__text); ::ui_lang_runtime::accessible(__text, ::ui_lang_runtime::StableId::new(&__a11y_key), ::ui_lang_runtime::Role::Label).logical_id_maybe(::core::cfg!(test).then_some(__a11y_key)).value(__text_value).into() };
#[cfg(test)]
drop(__ice_render_source);
__ice_rendered
}); __children.push({
#[cfg(test)]
let __ice_render_source_location = ::ui_lang_runtime::testing::Location::new("components/shell.ice", 263, 1, "rendered view node");
#[cfg(test)]
let __ice_render_source = ::ui_lang_runtime::testing::push_render_source(__ice_render_source_location);
let __ice_rendered: __IceElement<'_, __DucktapeMessage> = { let __a11y_key = format!("{}/@container:263", __ice_use_scope); let __container_content: __IceElement<'_, __DucktapeMessage> = {
#[cfg(test)]
let __ice_render_source_location = ::ui_lang_runtime::testing::Location::new("components/shell.ice", 264, 1, "rendered view node");
#[cfg(test)]
let __ice_render_source = ::ui_lang_runtime::testing::push_render_source(__ice_render_source_location);
let __ice_rendered: __IceElement<'_, __DucktapeMessage> = { let mut __children: ::std::vec::Vec<__IceElement<'_, __DucktapeMessage>> = ::std::vec::Vec::new(); __children.push({
#[cfg(test)]
let __ice_render_source_location = ::ui_lang_runtime::testing::Location::new("components/shell.ice", 265, 1, "rendered view node");
#[cfg(test)]
let __ice_render_source = ::ui_lang_runtime::testing::push_render_source(__ice_render_source_location);
let __ice_rendered: __IceElement<'_, __DucktapeMessage> = ::iced::widget::space().width(::iced::Fill).into();
#[cfg(test)]
drop(__ice_render_source);
__ice_rendered
}); __children.push({
#[cfg(test)]
let __ice_render_source_location = ::ui_lang_runtime::testing::Location::new("components/shell.ice", 266, 1, "rendered view node");
#[cfg(test)]
let __ice_render_source = ::ui_lang_runtime::testing::push_render_source(__ice_render_source_location);
let __ice_rendered: __IceElement<'_, __DucktapeMessage> = { let __a11y_key = format!("{}/@text:266", __ice_use_scope); let __text_value = (self.node_root_hash).to_string(); let __text = ::iced::widget::text(__text_value.clone()).size(((10.5) as f32).max(f32::EPSILON).min(f32::MAX)).wrapping(::iced::widget::text::Wrapping::None).font(::iced::Font { family: ::iced::font::Family::Name("Geist Mono"), weight: ::iced::font::Weight::Medium, stretch: ::iced::font::Stretch::Normal, style: ::iced::font::Style::Normal }).color(__ice_palette.colors[13]); let __text = ::ui_lang_runtime::selectable_text(__text); ::ui_lang_runtime::accessible(__text, ::ui_lang_runtime::StableId::new(&__a11y_key), ::ui_lang_runtime::Role::Label).logical_id_maybe(::core::cfg!(test).then_some(__a11y_key)).value(__text_value).into() };
#[cfg(test)]
drop(__ice_render_source);
__ice_rendered
}); let __child_count = __children.len(); let __children = __children.into_iter().map(|__child| ::ui_lang_runtime::bounded_fill_element(__child, __child_count, true)).collect::<::std::vec::Vec<_>>(); let __layout = ::iced::widget::row(__children).width(::iced::Fill); let __content = ::iced::widget::container(__layout); let __layout_content: __IceElement<'_, __DucktapeMessage> = __content.into(); let __a11y_key = format!("{}/@layout:264", __ice_use_scope); ::ui_lang_runtime::accessible(__layout_content, ::ui_lang_runtime::StableId::new(&__a11y_key), ::ui_lang_runtime::Role::GenericContainer).logical_id_maybe(::core::cfg!(test).then_some(__a11y_key)).into() };
#[cfg(test)]
drop(__ice_render_source);
__ice_rendered
}; let __container = {  ::iced::widget::container(__container_content).width(::iced::Fill).clip(true) }; ::ui_lang_runtime::accessible(__container, ::ui_lang_runtime::StableId::new(&__a11y_key), ::ui_lang_runtime::Role::GenericContainer).logical_id_maybe(::core::cfg!(test).then_some(__a11y_key)).into() };
#[cfg(test)]
drop(__ice_render_source);
__ice_rendered
}); let __child_count = __children.len(); let __children = __children.into_iter().map(|__child| ::ui_lang_runtime::bounded_fill_element(__child, __child_count, true)).collect::<::std::vec::Vec<_>>(); let __layout = ::iced::widget::row(__children).spacing(::ui_lang_runtime::bounded_spacing(14.0, __child_count)).width(::iced::Fill).align_y(::iced::alignment::Vertical::Center); let __content = ::iced::widget::container(__layout); let __layout_content: __IceElement<'_, __DucktapeMessage> = __content.into(); let __a11y_key = format!("{}/@layout:252", __ice_use_scope); ::ui_lang_runtime::accessible(__layout_content, ::ui_lang_runtime::StableId::new(&__a11y_key), ::ui_lang_runtime::Role::GenericContainer).logical_id_maybe(::core::cfg!(test).then_some(__a11y_key)).into() };
#[cfg(test)]
drop(__ice_render_source);
__ice_rendered
}); __children.push({
#[cfg(test)]
let __ice_render_source_location = ::ui_lang_runtime::testing::Location::new("components/shell.ice", 272, 1, "rendered view node");
#[cfg(test)]
let __ice_render_source = ::ui_lang_runtime::testing::push_render_source(__ice_render_source_location);
let __ice_rendered: __IceElement<'_, __DucktapeMessage> = { let mut __children: ::std::vec::Vec<__IceElement<'_, __DucktapeMessage>> = ::std::vec::Vec::new(); __children.push({
#[cfg(test)]
let __ice_render_source_location = ::ui_lang_runtime::testing::Location::new("components/shell.ice", 277, 1, "rendered view node");
#[cfg(test)]
let __ice_render_source = ::ui_lang_runtime::testing::push_render_source(__ice_render_source_location);
let __ice_rendered: __IceElement<'_, __DucktapeMessage> = { let __a11y_key = format!("{}/@text:277", __ice_use_scope); let __text_value = ("finality").to_string(); let __text = ::iced::widget::text(__text_value.clone()).size(((10.5) as f32).max(f32::EPSILON).min(f32::MAX)).wrapping(::iced::widget::text::Wrapping::None).font(::iced::Font { family: ::iced::font::Family::Name("Geist Mono"), weight: ::iced::font::Weight::Medium, stretch: ::iced::font::Stretch::Normal, style: ::iced::font::Style::Normal }).color(__ice_palette.colors[72]); let __text = ::ui_lang_runtime::selectable_text(__text); ::ui_lang_runtime::accessible(__text, ::ui_lang_runtime::StableId::new(&__a11y_key), ::ui_lang_runtime::Role::Label).logical_id_maybe(::core::cfg!(test).then_some(__a11y_key)).value(__text_value).into() };
#[cfg(test)]
drop(__ice_render_source);
__ice_rendered
}); __children.push({
#[cfg(test)]
let __ice_render_source_location = ::ui_lang_runtime::testing::Location::new("components/shell.ice", 283, 1, "rendered view node");
#[cfg(test)]
let __ice_render_source = ::ui_lang_runtime::testing::push_render_source(__ice_render_source_location);
let __ice_rendered: __IceElement<'_, __DucktapeMessage> = ::iced::widget::space().width(::iced::Fill).into();
#[cfg(test)]
drop(__ice_render_source);
__ice_rendered
}); __children.push({
#[cfg(test)]
let __ice_render_source_location = ::ui_lang_runtime::testing::Location::new("components/shell.ice", 284, 1, "rendered view node");
#[cfg(test)]
let __ice_render_source = ::ui_lang_runtime::testing::push_render_source(__ice_render_source_location);
let __ice_rendered: __IceElement<'_, __DucktapeMessage> = { let mut __children: ::std::vec::Vec<__IceElement<'_, __DucktapeMessage>> = ::std::vec::Vec::new(); __children.push({
#[cfg(test)]
let __ice_render_source_location = ::ui_lang_runtime::testing::Location::new("components/shell.ice", 285, 1, "rendered view node");
#[cfg(test)]
let __ice_render_source = ::ui_lang_runtime::testing::push_render_source(__ice_render_source_location);
let __ice_rendered: __IceElement<'_, __DucktapeMessage> = { let __a11y_key = format!("{}/@text:285", __ice_use_scope); let __text_value = ("view").to_string(); let __text = ::iced::widget::text(__text_value.clone()).size(((10.5) as f32).max(f32::EPSILON).min(f32::MAX)).wrapping(::iced::widget::text::Wrapping::None).font(::iced::Font { family: ::iced::font::Family::Name("Geist Mono"), weight: ::iced::font::Weight::Medium, stretch: ::iced::font::Stretch::Normal, style: ::iced::font::Style::Normal }).color(__ice_palette.colors[13]); let __text = ::ui_lang_runtime::selectable_text(__text); ::ui_lang_runtime::accessible(__text, ::ui_lang_runtime::StableId::new(&__a11y_key), ::ui_lang_runtime::Role::Label).logical_id_maybe(::core::cfg!(test).then_some(__a11y_key)).value(__text_value).into() };
#[cfg(test)]
drop(__ice_render_source);
__ice_rendered
}); __children.push({
#[cfg(test)]
let __ice_render_source_location = ::ui_lang_runtime::testing::Location::new("components/shell.ice", 291, 1, "rendered view node");
#[cfg(test)]
let __ice_render_source = ::ui_lang_runtime::testing::push_render_source(__ice_render_source_location);
let __ice_rendered: __IceElement<'_, __DucktapeMessage> = { let __a11y_key = format!("{}/@text:291", __ice_use_scope); let __text_value = (self.node_view_label).to_string(); let __text = ::iced::widget::text(__text_value.clone()).size(((10.5) as f32).max(f32::EPSILON).min(f32::MAX)).wrapping(::iced::widget::text::Wrapping::None).font(::iced::Font { family: ::iced::font::Family::Name("Geist Mono"), weight: ::iced::font::Weight::Medium, stretch: ::iced::font::Stretch::Normal, style: ::iced::font::Style::Normal }).color(__ice_palette.colors[13]); let __text = ::ui_lang_runtime::selectable_text(__text); ::ui_lang_runtime::accessible(__text, ::ui_lang_runtime::StableId::new(&__a11y_key), ::ui_lang_runtime::Role::Label).logical_id_maybe(::core::cfg!(test).then_some(__a11y_key)).value(__text_value).into() };
#[cfg(test)]
drop(__ice_render_source);
__ice_rendered
}); __children.push({
#[cfg(test)]
let __ice_render_source_location = ::ui_lang_runtime::testing::Location::new("components/shell.ice", 297, 1, "rendered view node");
#[cfg(test)]
let __ice_render_source = ::ui_lang_runtime::testing::push_render_source(__ice_render_source_location);
let __ice_rendered: __IceElement<'_, __DucktapeMessage> = { let __a11y_key = format!("{}/@text:297", __ice_use_scope); let __text_value = ("·").to_string(); let __text = ::iced::widget::text(__text_value.clone()).size(((10.5) as f32).max(f32::EPSILON).min(f32::MAX)).wrapping(::iced::widget::text::Wrapping::None).font(::iced::Font { family: ::iced::font::Family::Name("Geist Mono"), weight: ::iced::font::Weight::Medium, stretch: ::iced::font::Stretch::Normal, style: ::iced::font::Style::Normal }).color(__ice_palette.colors[72]); let __text = ::ui_lang_runtime::selectable_text(__text); ::ui_lang_runtime::accessible(__text, ::ui_lang_runtime::StableId::new(&__a11y_key), ::ui_lang_runtime::Role::Label).logical_id_maybe(::core::cfg!(test).then_some(__a11y_key)).value(__text_value).into() };
#[cfg(test)]
drop(__ice_render_source);
__ice_rendered
}); __children.push({
#[cfg(test)]
let __ice_render_source_location = ::ui_lang_runtime::testing::Location::new("components/shell.ice", 303, 1, "rendered view node");
#[cfg(test)]
let __ice_render_source = ::ui_lang_runtime::testing::push_render_source(__ice_render_source_location);
let __ice_rendered: __IceElement<'_, __DucktapeMessage> = { let __a11y_key = format!("{}/@text:303", __ice_use_scope); let __text_value = (self.node_reachable_label).to_string(); let __text = ::iced::widget::text(__text_value.clone()).size(((10.5) as f32).max(f32::EPSILON).min(f32::MAX)).wrapping(::iced::widget::text::Wrapping::None).font(::iced::Font { family: ::iced::font::Family::Name("Geist Mono"), weight: ::iced::font::Weight::Medium, stretch: ::iced::font::Stretch::Normal, style: ::iced::font::Style::Normal }).color(__ice_palette.colors[13]); let __text = ::ui_lang_runtime::selectable_text(__text); ::ui_lang_runtime::accessible(__text, ::ui_lang_runtime::StableId::new(&__a11y_key), ::ui_lang_runtime::Role::Label).logical_id_maybe(::core::cfg!(test).then_some(__a11y_key)).value(__text_value).into() };
#[cfg(test)]
drop(__ice_render_source);
__ice_rendered
}); __children.push({
#[cfg(test)]
let __ice_render_source_location = ::ui_lang_runtime::testing::Location::new("components/shell.ice", 309, 1, "rendered view node");
#[cfg(test)]
let __ice_render_source = ::ui_lang_runtime::testing::push_render_source(__ice_render_source_location);
let __ice_rendered: __IceElement<'_, __DucktapeMessage> = { let __a11y_key = format!("{}/@text:309", __ice_use_scope); let __text_value = ("/").to_string(); let __text = ::iced::widget::text(__text_value.clone()).size(((10.5) as f32).max(f32::EPSILON).min(f32::MAX)).wrapping(::iced::widget::text::Wrapping::None).font(::iced::Font { family: ::iced::font::Family::Name("Geist Mono"), weight: ::iced::font::Weight::Medium, stretch: ::iced::font::Stretch::Normal, style: ::iced::font::Style::Normal }).color(__ice_palette.colors[72]); let __text = ::ui_lang_runtime::selectable_text(__text); ::ui_lang_runtime::accessible(__text, ::ui_lang_runtime::StableId::new(&__a11y_key), ::ui_lang_runtime::Role::Label).logical_id_maybe(::core::cfg!(test).then_some(__a11y_key)).value(__text_value).into() };
#[cfg(test)]
drop(__ice_render_source);
__ice_rendered
}); __children.push({
#[cfg(test)]
let __ice_render_source_location = ::ui_lang_runtime::testing::Location::new("components/shell.ice", 315, 1, "rendered view node");
#[cfg(test)]
let __ice_render_source = ::ui_lang_runtime::testing::push_render_source(__ice_render_source_location);
let __ice_rendered: __IceElement<'_, __DucktapeMessage> = { let __a11y_key = format!("{}/@text:315", __ice_use_scope); let __text_value = (self.node_quorum_label).to_string(); let __text = ::iced::widget::text(__text_value.clone()).size(((10.5) as f32).max(f32::EPSILON).min(f32::MAX)).wrapping(::iced::widget::text::Wrapping::None).font(::iced::Font { family: ::iced::font::Family::Name("Geist Mono"), weight: ::iced::font::Weight::Medium, stretch: ::iced::font::Stretch::Normal, style: ::iced::font::Style::Normal }).color(__ice_palette.colors[13]); let __text = ::ui_lang_runtime::selectable_text(__text); ::ui_lang_runtime::accessible(__text, ::ui_lang_runtime::StableId::new(&__a11y_key), ::ui_lang_runtime::Role::Label).logical_id_maybe(::core::cfg!(test).then_some(__a11y_key)).value(__text_value).into() };
#[cfg(test)]
drop(__ice_render_source);
__ice_rendered
}); __children.push({
#[cfg(test)]
let __ice_render_source_location = ::ui_lang_runtime::testing::Location::new("components/shell.ice", 321, 1, "rendered view node");
#[cfg(test)]
let __ice_render_source = ::ui_lang_runtime::testing::push_render_source(__ice_render_source_location);
let __ice_rendered: __IceElement<'_, __DucktapeMessage> = { let __a11y_key = format!("{}/@text:321", __ice_use_scope); let __text_value = ("certs").to_string(); let __text = ::iced::widget::text(__text_value.clone()).size(((10.5) as f32).max(f32::EPSILON).min(f32::MAX)).wrapping(::iced::widget::text::Wrapping::None).font(::iced::Font { family: ::iced::font::Family::Name("Geist Mono"), weight: ::iced::font::Weight::Medium, stretch: ::iced::font::Stretch::Normal, style: ::iced::font::Style::Normal }).color(__ice_palette.colors[13]); let __text = ::ui_lang_runtime::selectable_text(__text); ::ui_lang_runtime::accessible(__text, ::ui_lang_runtime::StableId::new(&__a11y_key), ::ui_lang_runtime::Role::Label).logical_id_maybe(::core::cfg!(test).then_some(__a11y_key)).value(__text_value).into() };
#[cfg(test)]
drop(__ice_render_source);
__ice_rendered
}); let __child_count = __children.len(); let __children = __children.into_iter().map(|__child| ::ui_lang_runtime::bounded_fill_element(__child, __child_count, true)).collect::<::std::vec::Vec<_>>(); let __layout = ::iced::widget::row(__children).spacing(::ui_lang_runtime::bounded_spacing(4.0, __child_count)).align_y(::iced::alignment::Vertical::Center); let __content = ::iced::widget::container(__layout); let __layout_content: __IceElement<'_, __DucktapeMessage> = __content.into(); let __a11y_key = format!("{}/@layout:284", __ice_use_scope); ::ui_lang_runtime::accessible(__layout_content, ::ui_lang_runtime::StableId::new(&__a11y_key), ::ui_lang_runtime::Role::GenericContainer).logical_id_maybe(::core::cfg!(test).then_some(__a11y_key)).into() };
#[cfg(test)]
drop(__ice_render_source);
__ice_rendered
}); let __child_count = __children.len(); let __children = __children.into_iter().map(|__child| ::ui_lang_runtime::bounded_fill_element(__child, __child_count, true)).collect::<::std::vec::Vec<_>>(); let __layout = ::iced::widget::row(__children).spacing(::ui_lang_runtime::bounded_spacing(14.0, __child_count)).width(::iced::Fill).align_y(::iced::alignment::Vertical::Center); let __content = ::iced::widget::container(__layout); let __layout_content: __IceElement<'_, __DucktapeMessage> = __content.into(); let __a11y_key = format!("{}/@layout:272", __ice_use_scope); ::ui_lang_runtime::accessible(__layout_content, ::ui_lang_runtime::StableId::new(&__a11y_key), ::ui_lang_runtime::Role::GenericContainer).logical_id_maybe(::core::cfg!(test).then_some(__a11y_key)).into() };
#[cfg(test)]
drop(__ice_render_source);
__ice_rendered
}); __children.push({
#[cfg(test)]
let __ice_render_source_location = ::ui_lang_runtime::testing::Location::new("components/shell.ice", 327, 1, "rendered view node");
#[cfg(test)]
let __ice_render_source = ::ui_lang_runtime::testing::push_render_source(__ice_render_source_location);
let __ice_rendered: __IceElement<'_, __DucktapeMessage> = { let mut __children: ::std::vec::Vec<__IceElement<'_, __DucktapeMessage>> = ::std::vec::Vec::new(); __children.push({
#[cfg(test)]
let __ice_render_source_location = ::ui_lang_runtime::testing::Location::new("components/shell.ice", 332, 1, "rendered view node");
#[cfg(test)]
let __ice_render_source = ::ui_lang_runtime::testing::push_render_source(__ice_render_source_location);
let __ice_rendered: __IceElement<'_, __DucktapeMessage> = { let __a11y_key = format!("{}/@text:332", __ice_use_scope); let __text_value = ("last block").to_string(); let __text = ::iced::widget::text(__text_value.clone()).size(((10.5) as f32).max(f32::EPSILON).min(f32::MAX)).wrapping(::iced::widget::text::Wrapping::None).font(::iced::Font { family: ::iced::font::Family::Name("Geist Mono"), weight: ::iced::font::Weight::Medium, stretch: ::iced::font::Stretch::Normal, style: ::iced::font::Style::Normal }).color(__ice_palette.colors[72]); let __text = ::ui_lang_runtime::selectable_text(__text); ::ui_lang_runtime::accessible(__text, ::ui_lang_runtime::StableId::new(&__a11y_key), ::ui_lang_runtime::Role::Label).logical_id_maybe(::core::cfg!(test).then_some(__a11y_key)).value(__text_value).into() };
#[cfg(test)]
drop(__ice_render_source);
__ice_rendered
}); __children.push({
#[cfg(test)]
let __ice_render_source_location = ::ui_lang_runtime::testing::Location::new("components/shell.ice", 338, 1, "rendered view node");
#[cfg(test)]
let __ice_render_source = ::ui_lang_runtime::testing::push_render_source(__ice_render_source_location);
let __ice_rendered: __IceElement<'_, __DucktapeMessage> = ::iced::widget::space().width(::iced::Fill).into();
#[cfg(test)]
drop(__ice_render_source);
__ice_rendered
}); __children.push({
#[cfg(test)]
let __ice_render_source_location = ::ui_lang_runtime::testing::Location::new("components/shell.ice", 339, 1, "rendered view node");
#[cfg(test)]
let __ice_render_source = ::ui_lang_runtime::testing::push_render_source(__ice_render_source_location);
let __ice_rendered: __IceElement<'_, __DucktapeMessage> = { let __a11y_key = format!("{}/@text:339", __ice_use_scope); let __text_value = (crate::backend::relative_time(self.node_last_finalized, self.wall_now)).to_string(); let __text = ::iced::widget::text(__text_value.clone()).size(((10.5) as f32).max(f32::EPSILON).min(f32::MAX)).wrapping(::iced::widget::text::Wrapping::None).font(::iced::Font { family: ::iced::font::Family::Name("Geist Mono"), weight: ::iced::font::Weight::Medium, stretch: ::iced::font::Stretch::Normal, style: ::iced::font::Style::Normal }).color(__ice_palette.colors[13]); let __text = ::ui_lang_runtime::selectable_text(__text); ::ui_lang_runtime::accessible(__text, ::ui_lang_runtime::StableId::new(&__a11y_key), ::ui_lang_runtime::Role::Label).logical_id_maybe(::core::cfg!(test).then_some(__a11y_key)).value(__text_value).into() };
#[cfg(test)]
drop(__ice_render_source);
__ice_rendered
}); let __child_count = __children.len(); let __children = __children.into_iter().map(|__child| ::ui_lang_runtime::bounded_fill_element(__child, __child_count, true)).collect::<::std::vec::Vec<_>>(); let __layout = ::iced::widget::row(__children).spacing(::ui_lang_runtime::bounded_spacing(14.0, __child_count)).width(::iced::Fill).align_y(::iced::alignment::Vertical::Center); let __content = ::iced::widget::container(__layout); let __layout_content: __IceElement<'_, __DucktapeMessage> = __content.into(); let __a11y_key = format!("{}/@layout:327", __ice_use_scope); ::ui_lang_runtime::accessible(__layout_content, ::ui_lang_runtime::StableId::new(&__a11y_key), ::ui_lang_runtime::Role::GenericContainer).logical_id_maybe(::core::cfg!(test).then_some(__a11y_key)).into() };
#[cfg(test)]
drop(__ice_render_source);
__ice_rendered
}); let __child_count = __children.len(); let __children = __children.into_iter().map(|__child| ::ui_lang_runtime::bounded_fill_element(__child, __child_count, false)).collect::<::std::vec::Vec<_>>(); let __layout = ::iced::widget::column(__children).spacing(::ui_lang_runtime::bounded_spacing(6.0, __child_count)).width(::iced::Fill); let __content = ::iced::widget::container(__layout); let __layout_content: __IceElement<'_, __DucktapeMessage> = __content.into(); let __a11y_key = format!("{}/@layout:251", __ice_use_scope); ::ui_lang_runtime::accessible(__layout_content, ::ui_lang_runtime::StableId::new(&__a11y_key), ::ui_lang_runtime::Role::GenericContainer).logical_id_maybe(::core::cfg!(test).then_some(__a11y_key)).into() };
#[cfg(test)]
drop(__ice_render_source);
__ice_rendered
}); __children.push({
#[cfg(test)]
let __ice_render_source_location = ::ui_lang_runtime::testing::Location::new("components/shell.ice", 345, 1, "rendered view node");
#[cfg(test)]
let __ice_render_source = ::ui_lang_runtime::testing::push_render_source(__ice_render_source_location);
let __ice_rendered: __IceElement<'_, __DucktapeMessage> = { let mut __children: ::std::vec::Vec<__IceElement<'_, __DucktapeMessage>> = ::std::vec::Vec::new(); __children.push({
#[cfg(test)]
let __ice_render_source_location = ::ui_lang_runtime::testing::Location::new("components/shell.ice", 346, 1, "rendered view node");
#[cfg(test)]
let __ice_render_source = ::ui_lang_runtime::testing::push_render_source(__ice_render_source_location);
let __ice_rendered: __IceElement<'_, __DucktapeMessage> = { let __a11y_key = format!("{}/@container:346", __ice_use_scope); let __container_content: __IceElement<'_, __DucktapeMessage> = {
#[cfg(test)]
let __ice_render_source_location = ::ui_lang_runtime::testing::Location::new("components/shell.ice", 351, 1, "rendered view node");
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
let __ice_render_source_location = ::ui_lang_runtime::testing::Location::new("components/shell.ice", 352, 1, "rendered view node");
#[cfg(test)]
let __ice_render_source = ::ui_lang_runtime::testing::push_render_source(__ice_render_source_location);
let __ice_rendered: __IceElement<'_, __DucktapeMessage> = { let __a11y_key = format!("{}/@text:352", __ice_use_scope); let __text_value = ("This node verifies every record itself · it takes no one's word for it").to_string(); let __text = ::iced::widget::text(__text_value.clone()).size(((12.5) as f32).max(f32::EPSILON).min(f32::MAX)).width(::iced::Fill).color(__ice_palette.colors[71]); let __text = ::ui_lang_runtime::selectable_text(__text); ::ui_lang_runtime::accessible(__text, ::ui_lang_runtime::StableId::new(&__a11y_key), ::ui_lang_runtime::Role::Label).logical_id_maybe(::core::cfg!(test).then_some(__a11y_key)).value(__text_value).into() };
#[cfg(test)]
drop(__ice_render_source);
__ice_rendered
}); let __child_count = __children.len(); let __children = __children.into_iter().map(|__child| ::ui_lang_runtime::bounded_fill_element(__child, __child_count, false)).collect::<::std::vec::Vec<_>>(); let __layout = ::iced::widget::column(__children).spacing(::ui_lang_runtime::bounded_spacing(9.0, __child_count)).width(::iced::Fill); let __content = ::iced::widget::container(__layout); let __layout_content: __IceElement<'_, __DucktapeMessage> = __content.into(); let __a11y_key = format!("{}/@layout:345", __ice_use_scope); ::ui_lang_runtime::accessible(__layout_content, ::ui_lang_runtime::StableId::new(&__a11y_key), ::ui_lang_runtime::Role::GenericContainer).logical_id_maybe(::core::cfg!(test).then_some(__a11y_key)).into() };
#[cfg(test)]
drop(__ice_render_source);
__ice_rendered
}); let __child_count = __children.len(); let __children = __children.into_iter().map(|__child| ::ui_lang_runtime::bounded_fill_element(__child, __child_count, false)).collect::<::std::vec::Vec<_>>(); let __layout = ::iced::widget::column(__children).spacing(::ui_lang_runtime::bounded_spacing(11.0, __child_count)).width(::iced::Fill); let __content = ::iced::widget::container(__layout); let __layout_content: __IceElement<'_, __DucktapeMessage> = __content.into(); let __a11y_key = format!("{}/@layout:180", __ice_use_scope); ::ui_lang_runtime::accessible(__layout_content, ::ui_lang_runtime::StableId::new(&__a11y_key), ::ui_lang_runtime::Role::GenericContainer).logical_id_maybe(::core::cfg!(test).then_some(__a11y_key)).into() };
#[cfg(test)]
drop(__ice_render_source);
__ice_rendered
}; let __container = {  ::iced::widget::container(__container_content).id(::iced::widget::Id::from(__ice_node_scope.clone())).padding(::ui_lang_runtime::bounded_padding(13.0, 14.0, 13.0, 14.0)).width(284.0 as f32).style(move |__theme| { let mut __style = ::iced::widget::container::Style::default(); __style.background = ::std::option::Option::Some(::iced::Background::Color(__ice_palette.colors[3])); __style.border.color = __ice_palette.colors[39]; __style.border.width = 1.0 as f32; __style.border.radius = ::iced::border::Radius { top_left: ((13.0) as f32).max(0.0).min(f32::MAX), top_right: ((13.0) as f32).max(0.0).min(f32::MAX), bottom_right: ((13.0) as f32).max(0.0).min(f32::MAX), bottom_left: ((13.0) as f32).max(0.0).min(f32::MAX) }; __style.shadow.color = __ice_palette.colors[48]; __style.shadow.offset.y = 16.0 as f32; __style.shadow.blur_radius = 40.0 as f32; __style }) }; ::ui_lang_runtime::accessible(__container, ::ui_lang_runtime::StableId::new(&__a11y_key), ::ui_lang_runtime::Role::GenericContainer).logical_id_maybe(::core::cfg!(test).then_some(__a11y_key)).into() } };
#[cfg(test)]
drop(__ice_render_source);
__ice_rendered
}; __component_content }
pub(super) fn __ice_component_use_70(&self, __ice_palette: __IcePalette, __ice_use_scope: ::std::string::String) -> __IceElement<'_, __DucktapeMessage> { let __component_content: __IceElement<'_, __DucktapeMessage> = {
#[cfg(test)]
let __ice_render_source_location = ::ui_lang_runtime::testing::Location::new("components/shell.ice", 364, 1, "rendered view node");
#[cfg(test)]
let __ice_render_source = ::ui_lang_runtime::testing::push_render_source(__ice_render_source_location);
let __ice_rendered: __IceElement<'_, __DucktapeMessage> = { let __ice_node_scope = format!("{}/root", __ice_use_scope); { let mut __children: ::std::vec::Vec<__IceElement<'_, __DucktapeMessage>> = ::std::vec::Vec::new(); if (crate::backend::bell_worst_severity(::std::convert::AsRef::as_ref(&(self.bell_items))) == "danger") { __children.push({
#[cfg(test)]
let __ice_render_source_location = ::ui_lang_runtime::testing::Location::new("components/shell.ice", 367, 1, "rendered view node");
#[cfg(test)]
let __ice_render_source = ::ui_lang_runtime::testing::push_render_source(__ice_render_source_location);
let __ice_rendered: __IceElement<'_, __DucktapeMessage> = { let __a11y_key = format!("{}/@container:367", __ice_use_scope); let __container_content: __IceElement<'_, __DucktapeMessage> = {
#[cfg(test)]
let __ice_render_source_location = ::ui_lang_runtime::testing::Location::new("components/shell.ice", 377, 1, "rendered view node");
#[cfg(test)]
let __ice_render_source = ::ui_lang_runtime::testing::push_render_source(__ice_render_source_location);
let __ice_rendered: __IceElement<'_, __DucktapeMessage> = { let __a11y_key = format!("{}/@text:377", __ice_use_scope); let __text_value = (self.bell_unread).to_string(); let __text = ::iced::widget::text(__text_value.clone()).size(((9.0) as f32).max(f32::EPSILON).min(f32::MAX)).wrapping(::iced::widget::text::Wrapping::None).font(::iced::Font { family: ::iced::font::Family::Name("Geist Mono"), weight: ::iced::font::Weight::Semibold, stretch: ::iced::font::Stretch::Normal, style: ::iced::font::Style::Normal }).color(__ice_palette.colors[17]); let __text = ::ui_lang_runtime::selectable_text(__text); ::ui_lang_runtime::accessible(__text, ::ui_lang_runtime::StableId::new(&__a11y_key), ::ui_lang_runtime::Role::Label).logical_id_maybe(::core::cfg!(test).then_some(__a11y_key)).value(__text_value).into() };
#[cfg(test)]
drop(__ice_render_source);
__ice_rendered
}; let __container = {  ::iced::widget::container(__container_content).width(13.0 as f32).height(13.0 as f32).align_x(::iced::alignment::Horizontal::Center).align_y(::iced::alignment::Vertical::Center).style(move |__theme| { let mut __style = ::iced::widget::container::Style::default(); __style.background = ::std::option::Option::Some(::iced::Background::Color(__ice_palette.colors[83])); __style.border.color = __ice_palette.colors[3]; __style.border.width = 1.5 as f32; __style.border.radius = ::iced::border::Radius { top_left: ((7.0) as f32).max(0.0).min(f32::MAX), top_right: ((7.0) as f32).max(0.0).min(f32::MAX), bottom_right: ((7.0) as f32).max(0.0).min(f32::MAX), bottom_left: ((7.0) as f32).max(0.0).min(f32::MAX) }; __style }) }; ::ui_lang_runtime::accessible(__container, ::ui_lang_runtime::StableId::new(&__a11y_key), ::ui_lang_runtime::Role::GenericContainer).logical_id_maybe(::core::cfg!(test).then_some(__a11y_key)).into() };
#[cfg(test)]
drop(__ice_render_source);
__ice_rendered
}); } if ((!(crate::backend::bell_worst_severity(::std::convert::AsRef::as_ref(&(self.bell_items))) == "danger")) && (crate::backend::bell_worst_severity(::std::convert::AsRef::as_ref(&(self.bell_items))) == "warning")) { __children.push({
#[cfg(test)]
let __ice_render_source_location = ::ui_lang_runtime::testing::Location::new("components/shell.ice", 384, 1, "rendered view node");
#[cfg(test)]
let __ice_render_source = ::ui_lang_runtime::testing::push_render_source(__ice_render_source_location);
let __ice_rendered: __IceElement<'_, __DucktapeMessage> = { let __a11y_key = format!("{}/@container:384", __ice_use_scope); let __container_content: __IceElement<'_, __DucktapeMessage> = {
#[cfg(test)]
let __ice_render_source_location = ::ui_lang_runtime::testing::Location::new("components/shell.ice", 394, 1, "rendered view node");
#[cfg(test)]
let __ice_render_source = ::ui_lang_runtime::testing::push_render_source(__ice_render_source_location);
let __ice_rendered: __IceElement<'_, __DucktapeMessage> = { let __a11y_key = format!("{}/@text:394", __ice_use_scope); let __text_value = (self.bell_unread).to_string(); let __text = ::iced::widget::text(__text_value.clone()).size(((9.0) as f32).max(f32::EPSILON).min(f32::MAX)).wrapping(::iced::widget::text::Wrapping::None).font(::iced::Font { family: ::iced::font::Family::Name("Geist Mono"), weight: ::iced::font::Weight::Semibold, stretch: ::iced::font::Stretch::Normal, style: ::iced::font::Style::Normal }).color(__ice_palette.colors[17]); let __text = ::ui_lang_runtime::selectable_text(__text); ::ui_lang_runtime::accessible(__text, ::ui_lang_runtime::StableId::new(&__a11y_key), ::ui_lang_runtime::Role::Label).logical_id_maybe(::core::cfg!(test).then_some(__a11y_key)).value(__text_value).into() };
#[cfg(test)]
drop(__ice_render_source);
__ice_rendered
}; let __container = {  ::iced::widget::container(__container_content).width(13.0 as f32).height(13.0 as f32).align_x(::iced::alignment::Horizontal::Center).align_y(::iced::alignment::Vertical::Center).style(move |__theme| { let mut __style = ::iced::widget::container::Style::default(); __style.background = ::std::option::Option::Some(::iced::Background::Color(__ice_palette.colors[34])); __style.border.color = __ice_palette.colors[3]; __style.border.width = 1.5 as f32; __style.border.radius = ::iced::border::Radius { top_left: ((7.0) as f32).max(0.0).min(f32::MAX), top_right: ((7.0) as f32).max(0.0).min(f32::MAX), bottom_right: ((7.0) as f32).max(0.0).min(f32::MAX), bottom_left: ((7.0) as f32).max(0.0).min(f32::MAX) }; __style }) }; ::ui_lang_runtime::accessible(__container, ::ui_lang_runtime::StableId::new(&__a11y_key), ::ui_lang_runtime::Role::GenericContainer).logical_id_maybe(::core::cfg!(test).then_some(__a11y_key)).into() };
#[cfg(test)]
drop(__ice_render_source);
__ice_rendered
}); } if (!((crate::backend::bell_worst_severity(::std::convert::AsRef::as_ref(&(self.bell_items))) == "danger") || (crate::backend::bell_worst_severity(::std::convert::AsRef::as_ref(&(self.bell_items))) == "warning"))) { __children.push({
#[cfg(test)]
let __ice_render_source_location = ::ui_lang_runtime::testing::Location::new("components/shell.ice", 401, 1, "rendered view node");
#[cfg(test)]
let __ice_render_source = ::ui_lang_runtime::testing::push_render_source(__ice_render_source_location);
let __ice_rendered: __IceElement<'_, __DucktapeMessage> = { let __a11y_key = format!("{}/@container:401", __ice_use_scope); let __container_content: __IceElement<'_, __DucktapeMessage> = {
#[cfg(test)]
let __ice_render_source_location = ::ui_lang_runtime::testing::Location::new("components/shell.ice", 411, 1, "rendered view node");
#[cfg(test)]
let __ice_render_source = ::ui_lang_runtime::testing::push_render_source(__ice_render_source_location);
let __ice_rendered: __IceElement<'_, __DucktapeMessage> = { let __a11y_key = format!("{}/@text:411", __ice_use_scope); let __text_value = (self.bell_unread).to_string(); let __text = ::iced::widget::text(__text_value.clone()).size(((9.0) as f32).max(f32::EPSILON).min(f32::MAX)).wrapping(::iced::widget::text::Wrapping::None).font(::iced::Font { family: ::iced::font::Family::Name("Geist Mono"), weight: ::iced::font::Weight::Semibold, stretch: ::iced::font::Stretch::Normal, style: ::iced::font::Style::Normal }).color(__ice_palette.colors[17]); let __text = ::ui_lang_runtime::selectable_text(__text); ::ui_lang_runtime::accessible(__text, ::ui_lang_runtime::StableId::new(&__a11y_key), ::ui_lang_runtime::Role::Label).logical_id_maybe(::core::cfg!(test).then_some(__a11y_key)).value(__text_value).into() };
#[cfg(test)]
drop(__ice_render_source);
__ice_rendered
}; let __container = {  ::iced::widget::container(__container_content).width(13.0 as f32).height(13.0 as f32).align_x(::iced::alignment::Horizontal::Center).align_y(::iced::alignment::Vertical::Center).style(move |__theme| { let mut __style = ::iced::widget::container::Style::default(); __style.background = ::std::option::Option::Some(::iced::Background::Color(__ice_palette.colors[79])); __style.border.color = __ice_palette.colors[3]; __style.border.width = 1.5 as f32; __style.border.radius = ::iced::border::Radius { top_left: ((7.0) as f32).max(0.0).min(f32::MAX), top_right: ((7.0) as f32).max(0.0).min(f32::MAX), bottom_right: ((7.0) as f32).max(0.0).min(f32::MAX), bottom_left: ((7.0) as f32).max(0.0).min(f32::MAX) }; __style }) }; ::ui_lang_runtime::accessible(__container, ::ui_lang_runtime::StableId::new(&__a11y_key), ::ui_lang_runtime::Role::GenericContainer).logical_id_maybe(::core::cfg!(test).then_some(__a11y_key)).into() };
#[cfg(test)]
drop(__ice_render_source);
__ice_rendered
}); } let __child_count = __children.len(); let __children = __children.into_iter().map(|__child| ::ui_lang_runtime::bounded_fill_element(__child, __child_count, false)).collect::<::std::vec::Vec<_>>(); let __layout = ::iced::widget::column(__children); let __content = ::iced::widget::container(__layout); let __layout_content: __IceElement<'_, __DucktapeMessage> = __content.into(); let __a11y_key = __ice_node_scope.as_str(); ::ui_lang_runtime::accessible(__layout_content, ::ui_lang_runtime::StableId::new(&__a11y_key), ::ui_lang_runtime::Role::GenericContainer).logical_id_maybe(::core::cfg!(test).then_some(__a11y_key)).into() } };
#[cfg(test)]
drop(__ice_render_source);
__ice_rendered
}; __component_content }
pub(super) fn __ice_component_use_71(&self, __ice_palette: __IcePalette, __ice_use_scope: ::std::string::String) -> __IceElement<'_, __DucktapeMessage> { let __component_content: __IceElement<'_, __DucktapeMessage> = {
#[cfg(test)]
let __ice_render_source_location = ::ui_lang_runtime::testing::Location::new("components/shell.ice", 364, 1, "rendered view node");
#[cfg(test)]
let __ice_render_source = ::ui_lang_runtime::testing::push_render_source(__ice_render_source_location);
let __ice_rendered: __IceElement<'_, __DucktapeMessage> = { let __ice_node_scope = format!("{}/root", __ice_use_scope); { let mut __children: ::std::vec::Vec<__IceElement<'_, __DucktapeMessage>> = ::std::vec::Vec::new(); if (crate::backend::bell_worst_severity(::std::convert::AsRef::as_ref(&(self.bell_items))) == "danger") { __children.push({
#[cfg(test)]
let __ice_render_source_location = ::ui_lang_runtime::testing::Location::new("components/shell.ice", 367, 1, "rendered view node");
#[cfg(test)]
let __ice_render_source = ::ui_lang_runtime::testing::push_render_source(__ice_render_source_location);
let __ice_rendered: __IceElement<'_, __DucktapeMessage> = { let __a11y_key = format!("{}/@container:367", __ice_use_scope); let __container_content: __IceElement<'_, __DucktapeMessage> = {
#[cfg(test)]
let __ice_render_source_location = ::ui_lang_runtime::testing::Location::new("components/shell.ice", 377, 1, "rendered view node");
#[cfg(test)]
let __ice_render_source = ::ui_lang_runtime::testing::push_render_source(__ice_render_source_location);
let __ice_rendered: __IceElement<'_, __DucktapeMessage> = { let __a11y_key = format!("{}/@text:377", __ice_use_scope); let __text_value = (self.bell_unread).to_string(); let __text = ::iced::widget::text(__text_value.clone()).size(((9.0) as f32).max(f32::EPSILON).min(f32::MAX)).wrapping(::iced::widget::text::Wrapping::None).font(::iced::Font { family: ::iced::font::Family::Name("Geist Mono"), weight: ::iced::font::Weight::Semibold, stretch: ::iced::font::Stretch::Normal, style: ::iced::font::Style::Normal }).color(__ice_palette.colors[17]); let __text = ::ui_lang_runtime::selectable_text(__text); ::ui_lang_runtime::accessible(__text, ::ui_lang_runtime::StableId::new(&__a11y_key), ::ui_lang_runtime::Role::Label).logical_id_maybe(::core::cfg!(test).then_some(__a11y_key)).value(__text_value).into() };
#[cfg(test)]
drop(__ice_render_source);
__ice_rendered
}; let __container = {  ::iced::widget::container(__container_content).width(18.0 as f32).height(13.0 as f32).align_x(::iced::alignment::Horizontal::Center).align_y(::iced::alignment::Vertical::Center).style(move |__theme| { let mut __style = ::iced::widget::container::Style::default(); __style.background = ::std::option::Option::Some(::iced::Background::Color(__ice_palette.colors[83])); __style.border.color = __ice_palette.colors[3]; __style.border.width = 1.5 as f32; __style.border.radius = ::iced::border::Radius { top_left: ((7.0) as f32).max(0.0).min(f32::MAX), top_right: ((7.0) as f32).max(0.0).min(f32::MAX), bottom_right: ((7.0) as f32).max(0.0).min(f32::MAX), bottom_left: ((7.0) as f32).max(0.0).min(f32::MAX) }; __style }) }; ::ui_lang_runtime::accessible(__container, ::ui_lang_runtime::StableId::new(&__a11y_key), ::ui_lang_runtime::Role::GenericContainer).logical_id_maybe(::core::cfg!(test).then_some(__a11y_key)).into() };
#[cfg(test)]
drop(__ice_render_source);
__ice_rendered
}); } if ((!(crate::backend::bell_worst_severity(::std::convert::AsRef::as_ref(&(self.bell_items))) == "danger")) && (crate::backend::bell_worst_severity(::std::convert::AsRef::as_ref(&(self.bell_items))) == "warning")) { __children.push({
#[cfg(test)]
let __ice_render_source_location = ::ui_lang_runtime::testing::Location::new("components/shell.ice", 384, 1, "rendered view node");
#[cfg(test)]
let __ice_render_source = ::ui_lang_runtime::testing::push_render_source(__ice_render_source_location);
let __ice_rendered: __IceElement<'_, __DucktapeMessage> = { let __a11y_key = format!("{}/@container:384", __ice_use_scope); let __container_content: __IceElement<'_, __DucktapeMessage> = {
#[cfg(test)]
let __ice_render_source_location = ::ui_lang_runtime::testing::Location::new("components/shell.ice", 394, 1, "rendered view node");
#[cfg(test)]
let __ice_render_source = ::ui_lang_runtime::testing::push_render_source(__ice_render_source_location);
let __ice_rendered: __IceElement<'_, __DucktapeMessage> = { let __a11y_key = format!("{}/@text:394", __ice_use_scope); let __text_value = (self.bell_unread).to_string(); let __text = ::iced::widget::text(__text_value.clone()).size(((9.0) as f32).max(f32::EPSILON).min(f32::MAX)).wrapping(::iced::widget::text::Wrapping::None).font(::iced::Font { family: ::iced::font::Family::Name("Geist Mono"), weight: ::iced::font::Weight::Semibold, stretch: ::iced::font::Stretch::Normal, style: ::iced::font::Style::Normal }).color(__ice_palette.colors[17]); let __text = ::ui_lang_runtime::selectable_text(__text); ::ui_lang_runtime::accessible(__text, ::ui_lang_runtime::StableId::new(&__a11y_key), ::ui_lang_runtime::Role::Label).logical_id_maybe(::core::cfg!(test).then_some(__a11y_key)).value(__text_value).into() };
#[cfg(test)]
drop(__ice_render_source);
__ice_rendered
}; let __container = {  ::iced::widget::container(__container_content).width(18.0 as f32).height(13.0 as f32).align_x(::iced::alignment::Horizontal::Center).align_y(::iced::alignment::Vertical::Center).style(move |__theme| { let mut __style = ::iced::widget::container::Style::default(); __style.background = ::std::option::Option::Some(::iced::Background::Color(__ice_palette.colors[34])); __style.border.color = __ice_palette.colors[3]; __style.border.width = 1.5 as f32; __style.border.radius = ::iced::border::Radius { top_left: ((7.0) as f32).max(0.0).min(f32::MAX), top_right: ((7.0) as f32).max(0.0).min(f32::MAX), bottom_right: ((7.0) as f32).max(0.0).min(f32::MAX), bottom_left: ((7.0) as f32).max(0.0).min(f32::MAX) }; __style }) }; ::ui_lang_runtime::accessible(__container, ::ui_lang_runtime::StableId::new(&__a11y_key), ::ui_lang_runtime::Role::GenericContainer).logical_id_maybe(::core::cfg!(test).then_some(__a11y_key)).into() };
#[cfg(test)]
drop(__ice_render_source);
__ice_rendered
}); } if (!((crate::backend::bell_worst_severity(::std::convert::AsRef::as_ref(&(self.bell_items))) == "danger") || (crate::backend::bell_worst_severity(::std::convert::AsRef::as_ref(&(self.bell_items))) == "warning"))) { __children.push({
#[cfg(test)]
let __ice_render_source_location = ::ui_lang_runtime::testing::Location::new("components/shell.ice", 401, 1, "rendered view node");
#[cfg(test)]
let __ice_render_source = ::ui_lang_runtime::testing::push_render_source(__ice_render_source_location);
let __ice_rendered: __IceElement<'_, __DucktapeMessage> = { let __a11y_key = format!("{}/@container:401", __ice_use_scope); let __container_content: __IceElement<'_, __DucktapeMessage> = {
#[cfg(test)]
let __ice_render_source_location = ::ui_lang_runtime::testing::Location::new("components/shell.ice", 411, 1, "rendered view node");
#[cfg(test)]
let __ice_render_source = ::ui_lang_runtime::testing::push_render_source(__ice_render_source_location);
let __ice_rendered: __IceElement<'_, __DucktapeMessage> = { let __a11y_key = format!("{}/@text:411", __ice_use_scope); let __text_value = (self.bell_unread).to_string(); let __text = ::iced::widget::text(__text_value.clone()).size(((9.0) as f32).max(f32::EPSILON).min(f32::MAX)).wrapping(::iced::widget::text::Wrapping::None).font(::iced::Font { family: ::iced::font::Family::Name("Geist Mono"), weight: ::iced::font::Weight::Semibold, stretch: ::iced::font::Stretch::Normal, style: ::iced::font::Style::Normal }).color(__ice_palette.colors[17]); let __text = ::ui_lang_runtime::selectable_text(__text); ::ui_lang_runtime::accessible(__text, ::ui_lang_runtime::StableId::new(&__a11y_key), ::ui_lang_runtime::Role::Label).logical_id_maybe(::core::cfg!(test).then_some(__a11y_key)).value(__text_value).into() };
#[cfg(test)]
drop(__ice_render_source);
__ice_rendered
}; let __container = {  ::iced::widget::container(__container_content).width(18.0 as f32).height(13.0 as f32).align_x(::iced::alignment::Horizontal::Center).align_y(::iced::alignment::Vertical::Center).style(move |__theme| { let mut __style = ::iced::widget::container::Style::default(); __style.background = ::std::option::Option::Some(::iced::Background::Color(__ice_palette.colors[79])); __style.border.color = __ice_palette.colors[3]; __style.border.width = 1.5 as f32; __style.border.radius = ::iced::border::Radius { top_left: ((7.0) as f32).max(0.0).min(f32::MAX), top_right: ((7.0) as f32).max(0.0).min(f32::MAX), bottom_right: ((7.0) as f32).max(0.0).min(f32::MAX), bottom_left: ((7.0) as f32).max(0.0).min(f32::MAX) }; __style }) }; ::ui_lang_runtime::accessible(__container, ::ui_lang_runtime::StableId::new(&__a11y_key), ::ui_lang_runtime::Role::GenericContainer).logical_id_maybe(::core::cfg!(test).then_some(__a11y_key)).into() };
#[cfg(test)]
drop(__ice_render_source);
__ice_rendered
}); } let __child_count = __children.len(); let __children = __children.into_iter().map(|__child| ::ui_lang_runtime::bounded_fill_element(__child, __child_count, false)).collect::<::std::vec::Vec<_>>(); let __layout = ::iced::widget::column(__children); let __content = ::iced::widget::container(__layout); let __layout_content: __IceElement<'_, __DucktapeMessage> = __content.into(); let __a11y_key = __ice_node_scope.as_str(); ::ui_lang_runtime::accessible(__layout_content, ::ui_lang_runtime::StableId::new(&__a11y_key), ::ui_lang_runtime::Role::GenericContainer).logical_id_maybe(::core::cfg!(test).then_some(__a11y_key)).into() } };
#[cfg(test)]
drop(__ice_render_source);
__ice_rendered
}; __component_content }
pub(super) fn __ice_component_use_72(&self, __ice_palette: __IcePalette, __ice_use_scope: ::std::string::String, __ice_cb_1: impl Fn() -> __DucktapeMessage + Clone + 'static, __ice_cb_2: impl Fn() -> __DucktapeMessage + Clone + 'static) -> __IceElement<'_, __DucktapeMessage> { let __component_content: __IceElement<'_, __DucktapeMessage> = {
#[cfg(test)]
let __ice_render_source_location = ::ui_lang_runtime::testing::Location::new("components/shell.ice", 425, 1, "rendered view node");
#[cfg(test)]
let __ice_render_source = ::ui_lang_runtime::testing::push_render_source(__ice_render_source_location);
let __ice_rendered: __IceElement<'_, __DucktapeMessage> = { let __ice_node_scope = format!("{}/root", __ice_use_scope); { let mut __children: ::std::vec::Vec<__IceElement<'_, __DucktapeMessage>> = ::std::vec::Vec::new(); __children.push({
#[cfg(test)]
let __ice_render_source_location = ::ui_lang_runtime::testing::Location::new("components/shell.ice", 429, 1, "rendered view node");
#[cfg(test)]
let __ice_render_source = ::ui_lang_runtime::testing::push_render_source(__ice_render_source_location);
let __ice_rendered: __IceElement<'_, __DucktapeMessage> = { let __a11y_key = format!("{}/@container:429", __ice_use_scope); let __container_content: __IceElement<'_, __DucktapeMessage> = {
#[cfg(test)]
let __ice_render_source_location = ::ui_lang_runtime::testing::Location::new("components/shell.ice", 436, 1, "rendered view node");
#[cfg(test)]
let __ice_render_source = ::ui_lang_runtime::testing::push_render_source(__ice_render_source_location);
let __ice_rendered: __IceElement<'_, __DucktapeMessage> = { let mut __children: ::std::vec::Vec<__IceElement<'_, __DucktapeMessage>> = ::std::vec::Vec::new(); __children.push({
#[cfg(test)]
let __ice_render_source_location = ::ui_lang_runtime::testing::Location::new("components/shell.ice", 442, 1, "rendered view node");
#[cfg(test)]
let __ice_render_source = ::ui_lang_runtime::testing::push_render_source(__ice_render_source_location);
let __ice_rendered: __IceElement<'_, __DucktapeMessage> = { let __ice_component_04e6574776f726b43686970_scope_3362 = format!("{}/NetworkChip@3362", __ice_use_scope); ::ui_lang_runtime::rev_memo(378u64, (self.__ice_rev[124], __ice_palette.name), ::ui_lang_runtime::grow_stack(|| self.__ice_component_use_63(__ice_palette, __ice_component_04e6574776f726b43686970_scope_3362, (__ice_cb_1).clone()))).into() };
#[cfg(test)]
drop(__ice_render_source);
__ice_rendered
}); __children.push({
#[cfg(test)]
let __ice_render_source_location = ::ui_lang_runtime::testing::Location::new("components/shell.ice", 445, 1, "rendered view node");
#[cfg(test)]
let __ice_render_source = ::ui_lang_runtime::testing::push_render_source(__ice_render_source_location);
let __ice_rendered: __IceElement<'_, __DucktapeMessage> = ::iced::widget::space().width(::iced::Fill).into();
#[cfg(test)]
drop(__ice_render_source);
__ice_rendered
}); __children.push({
#[cfg(test)]
let __ice_render_source_location = ::ui_lang_runtime::testing::Location::new("components/shell.ice", 446, 1, "rendered view node");
#[cfg(test)]
let __ice_render_source = ::ui_lang_runtime::testing::push_render_source(__ice_render_source_location);
let __ice_rendered: __IceElement<'_, __DucktapeMessage> = { let mut __children: ::std::vec::Vec<__IceElement<'_, __DucktapeMessage>> = ::std::vec::Vec::new(); __children.push({
#[cfg(test)]
let __ice_render_source_location = ::ui_lang_runtime::testing::Location::new("components/shell.ice", 455, 1, "rendered view node");
#[cfg(test)]
let __ice_render_source = ::ui_lang_runtime::testing::push_render_source(__ice_render_source_location);
let __ice_rendered: __IceElement<'_, __DucktapeMessage> = { let __tooltip_content: __IceElement<'_, __DucktapeMessage> = {
#[cfg(test)]
let __ice_render_source_location = ::ui_lang_runtime::testing::Location::new("components/shell.ice", 462, 1, "rendered view node");
#[cfg(test)]
let __ice_render_source = ::ui_lang_runtime::testing::push_render_source(__ice_render_source_location);
let __ice_rendered: __IceElement<'_, __DucktapeMessage> = { let __ice_component_053746174757350696c6c_scope_3382 = format!("{}/StatusPill@3382", __ice_use_scope); ::ui_lang_runtime::rev_memo(382u64, (self.__ice_rev[19], self.__ice_rev[7], self.__ice_rev[9], __ice_palette.name), ::ui_lang_runtime::grow_stack(|| self.__ice_component_use_65(__ice_palette, __ice_component_053746174757350696c6c_scope_3382))).into() };
#[cfg(test)]
drop(__ice_render_source);
__ice_rendered
}; let __tooltip_tip: __IceElement<'_, __DucktapeMessage> = {
#[cfg(test)]
let __ice_render_source_location = ::ui_lang_runtime::testing::Location::new("components/shell.ice", 463, 1, "rendered view node");
#[cfg(test)]
let __ice_render_source = ::ui_lang_runtime::testing::push_render_source(__ice_render_source_location);
let __ice_rendered: __IceElement<'_, __DucktapeMessage> = { let __a11y_key = format!("{}/@container:463", __ice_use_scope); let __container_content: __IceElement<'_, __DucktapeMessage> = {
#[cfg(test)]
let __ice_render_source_location = ::ui_lang_runtime::testing::Location::new("components/shell.ice", 464, 1, "rendered view node");
#[cfg(test)]
let __ice_render_source = ::ui_lang_runtime::testing::push_render_source(__ice_render_source_location);
let __ice_rendered: __IceElement<'_, __DucktapeMessage> = { let __ice_component_053746174757343617264_scope_3384 = format!("{}/StatusCard@3384", __ice_use_scope); ::ui_lang_runtime::rev_memo(384u64, ([self.__ice_rev[14], self.__ice_rev[19], self.__ice_rev[3], self.__ice_rev[56], self.__ice_rev[57], self.__ice_rev[7], self.__ice_rev[85], self.__ice_rev[87], self.__ice_rev[90], self.__ice_rev[92], self.__ice_rev[93], self.__ice_rev[97], self.__ice_rev[98], self.__ice_rev[99], self.__ice_rev[9], ], __ice_palette.name), ::ui_lang_runtime::grow_stack(|| self.__ice_component_use_67(__ice_palette, __ice_component_053746174757343617264_scope_3384))).into() };
#[cfg(test)]
drop(__ice_render_source);
__ice_rendered
}; let __container = {  ::iced::widget::container(__container_content).padding(::ui_lang_runtime::bounded_padding(0.0, 13.0, 0.0, 0.0)) }; ::ui_lang_runtime::accessible(__container, ::ui_lang_runtime::StableId::new(&__a11y_key), ::ui_lang_runtime::Role::GenericContainer).logical_id_maybe(::core::cfg!(test).then_some(__a11y_key)).into() };
#[cfg(test)]
drop(__ice_render_source);
__ice_rendered
}; ::iced::widget::tooltip(__tooltip_content, __tooltip_tip, ::iced::widget::tooltip::Position::Bottom).gap(::ui_lang_runtime::bounded_table_metric(13.5, 1)).padding(::ui_lang_runtime::bounded_table_metric(0.0, 1)).delay(::std::time::Duration::from_millis(u64::try_from(90).unwrap_or(0))).snap_within_viewport(true).style(move |__theme| { let mut __style = ::iced::widget::container::transparent(__theme); __style }).into() };
#[cfg(test)]
drop(__ice_render_source);
__ice_rendered
}); __children.push({
#[cfg(test)]
let __ice_render_source_location = ::ui_lang_runtime::testing::Location::new("components/shell.ice", 478, 1, "rendered view node");
#[cfg(test)]
let __ice_render_source = ::ui_lang_runtime::testing::push_render_source(__ice_render_source_location);
let __ice_rendered: __IceElement<'_, __DucktapeMessage> = { let mut __children: ::std::vec::Vec<__IceElement<'_, __DucktapeMessage>> = ::std::vec::Vec::new(); __children.push({
#[cfg(test)]
let __ice_render_source_location = ::ui_lang_runtime::testing::Location::new("components/shell.ice", 479, 1, "rendered view node");
#[cfg(test)]
let __ice_render_source = ::ui_lang_runtime::testing::push_render_source(__ice_render_source_location);
let __ice_rendered: __IceElement<'_, __DucktapeMessage> = { let __a11y_key = format!("{}/@button:479", __ice_use_scope); let __a11y_id = ::ui_lang_runtime::StableId::new(&__a11y_key); let __disabled = false; let __activate = (__ice_cb_2)(); let __button_content: __IceElement<'_, __DucktapeMessage> = {
#[cfg(test)]
let __ice_render_source_location = ::ui_lang_runtime::testing::Location::new("components/shell.ice", 484, 1, "rendered view node");
#[cfg(test)]
let __ice_render_source = ::ui_lang_runtime::testing::push_render_source(__ice_render_source_location);
let __ice_rendered: __IceElement<'_, __DucktapeMessage> = { let mut __children: ::std::vec::Vec<__IceElement<'_, __DucktapeMessage>> = ::std::vec::Vec::new(); if (self.bell_unread > 0) { __children.push({
#[cfg(test)]
let __ice_render_source_location = ::ui_lang_runtime::testing::Location::new("components/shell.ice", 486, 1, "rendered view node");
#[cfg(test)]
let __ice_render_source = ::ui_lang_runtime::testing::push_render_source(__ice_render_source_location);
let __ice_rendered: __IceElement<'_, __DucktapeMessage> = ::ui_lang_runtime::grow_stack(|| self.__ice_component_use_68(__ice_palette, format!("{}/Icon@3406", __ice_use_scope)));
#[cfg(test)]
drop(__ice_render_source);
__ice_rendered
}); } if (self.bell_unread <= 0) { __children.push({
#[cfg(test)]
let __ice_render_source_location = ::ui_lang_runtime::testing::Location::new("components/shell.ice", 492, 1, "rendered view node");
#[cfg(test)]
let __ice_render_source = ::ui_lang_runtime::testing::push_render_source(__ice_render_source_location);
let __ice_rendered: __IceElement<'_, __DucktapeMessage> = ::ui_lang_runtime::grow_stack(|| self.__ice_component_use_69(__ice_palette, format!("{}/Icon@3412", __ice_use_scope)));
#[cfg(test)]
drop(__ice_render_source);
__ice_rendered
}); } let __child_count = __children.len(); let __children = __children.into_iter().map(|__child| ::ui_lang_runtime::bounded_fill_element(__child, __child_count, false)).collect::<::std::vec::Vec<_>>(); let __layout = ::iced::widget::column(__children).align_x(::iced::alignment::Horizontal::Center); let __content = ::iced::widget::container(__layout); let __layout_content: __IceElement<'_, __DucktapeMessage> = __content.into(); let __a11y_key = format!("{}/@layout:484", __ice_use_scope); ::ui_lang_runtime::accessible(__layout_content, ::ui_lang_runtime::StableId::new(&__a11y_key), ::ui_lang_runtime::Role::GenericContainer).logical_id_maybe(::core::cfg!(test).then_some(__a11y_key)).into() };
#[cfg(test)]
drop(__ice_render_source);
__ice_rendered
}; let __button = ::iced::widget::button(__button_content).padding(5.0 as f32).on_press_maybe(if __disabled { None } else { Some(__activate.clone()) }).style(move |__theme, __status| { let mut __style = ::iced::widget::button::primary(__theme, __status); let __background: Option<::iced::Color> = match __status { ::iced::widget::button::Status::Hovered => Some(__ice_palette.colors[14]), ::iced::widget::button::Status::Pressed => Some(__ice_palette.colors[39]), ::iced::widget::button::Status::Disabled => Some(::iced::Color::from_rgba8(0, 0, 0, 0.000000)), _ => Some(::iced::Color::from_rgba8(0, 0, 0, 0.000000)) }; if let Some(__background) = __background { __style.background = Some(::iced::Background::Color(__background)); } __style.text_color = __ice_palette.colors[4]; __style.border.radius = 7.0.into(); __style.background = ::std::option::Option::Some(::iced::Background::Color(::iced::Color::from_rgba8(0, 0, 0, 0.000000))); __style.text_color = __ice_palette.colors[5]; __style.border.color = ::iced::Color::from_rgba8(0, 0, 0, 0.000000); __style.border.width = 1.0 as f32; __style.border.radius = ::iced::border::Radius { top_left: ((7.0) as f32).max(0.0).min(f32::MAX), top_right: ((7.0) as f32).max(0.0).min(f32::MAX), bottom_right: ((7.0) as f32).max(0.0).min(f32::MAX), bottom_left: ((7.0) as f32).max(0.0).min(f32::MAX) }; match __status { ::iced::widget::button::Status::Hovered => { __style.background = ::std::option::Option::Some(::iced::Background::Color(__ice_palette.colors[3])); __style.text_color = __ice_palette.colors[4]; __style.border.color = ::iced::Color::from_rgba8(0, 0, 0, 0.000000); } ::iced::widget::button::Status::Pressed => { __style.background = ::std::option::Option::Some(::iced::Background::Color(__ice_palette.colors[56])); } _ => {} } if matches!(__status, ::iced::widget::button::Status::Disabled) { if let Some(::iced::Background::Color(mut __color)) = __style.background { __color.a *= 0.5; __style.background = Some(::iced::Background::Color(__color)); } __style.text_color.a *= 0.5; } __style }); ::ui_lang_runtime::accessible(__button, __a11y_id, ::ui_lang_runtime::Role::Button).logical_id_maybe(::core::cfg!(test).then_some(&*__a11y_key)).focus_id(::iced::widget::Id::from(__a11y_key)).label("Alerts".to_owned()).disabled(__disabled).on_activate_maybe(if __disabled { None } else { Some(__activate) }).focus_ring(__ice_palette.colors[42], 7.0).into() };
#[cfg(test)]
drop(__ice_render_source);
__ice_rendered
}); if ((self.bell_unread > 0) && (self.bell_unread < 10)) { __children.push({
#[cfg(test)]
let __ice_render_source_location = ::ui_lang_runtime::testing::Location::new("components/shell.ice", 504, 1, "rendered view node");
#[cfg(test)]
let __ice_render_source = ::ui_lang_runtime::testing::push_render_source(__ice_render_source_location);
let __ice_rendered: __IceElement<'_, __DucktapeMessage> = { let __pin_content: __IceElement<'_, __DucktapeMessage> = {
#[cfg(test)]
let __ice_render_source_location = ::ui_lang_runtime::testing::Location::new("components/shell.ice", 505, 1, "rendered view node");
#[cfg(test)]
let __ice_render_source = ::ui_lang_runtime::testing::push_render_source(__ice_render_source_location);
let __ice_rendered: __IceElement<'_, __DucktapeMessage> = { let __ice_component_042656c6c4261646765_scope_3425 = format!("{}/BellBadge@3425", __ice_use_scope); ::ui_lang_runtime::rev_memo(394u64, (self.__ice_rev[106], self.__ice_rev[107], __ice_palette.name), ::ui_lang_runtime::grow_stack(|| self.__ice_component_use_70(__ice_palette, __ice_component_042656c6c4261646765_scope_3425))).into() };
#[cfg(test)]
drop(__ice_render_source);
__ice_rendered
}; ::iced::widget::pin(__pin_content).x(12.0 as f32).y(1.0 as f32).into() };
#[cfg(test)]
drop(__ice_render_source);
__ice_rendered
}); } if (self.bell_unread > 9) { __children.push({
#[cfg(test)]
let __ice_render_source_location = ::ui_lang_runtime::testing::Location::new("components/shell.ice", 511, 1, "rendered view node");
#[cfg(test)]
let __ice_render_source = ::ui_lang_runtime::testing::push_render_source(__ice_render_source_location);
let __ice_rendered: __IceElement<'_, __DucktapeMessage> = { let __pin_content: __IceElement<'_, __DucktapeMessage> = {
#[cfg(test)]
let __ice_render_source_location = ::ui_lang_runtime::testing::Location::new("components/shell.ice", 512, 1, "rendered view node");
#[cfg(test)]
let __ice_render_source = ::ui_lang_runtime::testing::push_render_source(__ice_render_source_location);
let __ice_rendered: __IceElement<'_, __DucktapeMessage> = { let __ice_component_042656c6c4261646765_scope_3432 = format!("{}/BellBadge@3432", __ice_use_scope); ::ui_lang_runtime::rev_memo(397u64, (self.__ice_rev[106], self.__ice_rev[107], __ice_palette.name), ::ui_lang_runtime::grow_stack(|| self.__ice_component_use_71(__ice_palette, __ice_component_042656c6c4261646765_scope_3432))).into() };
#[cfg(test)]
drop(__ice_render_source);
__ice_rendered
}; ::iced::widget::pin(__pin_content).x(7.0 as f32).y(1.0 as f32).into() };
#[cfg(test)]
drop(__ice_render_source);
__ice_rendered
}); } let __layout = ::ui_lang_runtime::zstack(__children).width(26.0 as f32).height(24.0 as f32); let __content = ::iced::widget::container(__layout); let __layout_content: __IceElement<'_, __DucktapeMessage> = __content.into(); let __a11y_key = format!("{}/@layout:478", __ice_use_scope); ::ui_lang_runtime::accessible(__layout_content, ::ui_lang_runtime::StableId::new(&__a11y_key), ::ui_lang_runtime::Role::GenericContainer).logical_id_maybe(::core::cfg!(test).then_some(__a11y_key)).into() };
#[cfg(test)]
drop(__ice_render_source);
__ice_rendered
}); let __child_count = __children.len(); let __children = __children.into_iter().map(|__child| ::ui_lang_runtime::bounded_fill_element(__child, __child_count, true)).collect::<::std::vec::Vec<_>>(); let __layout = ::iced::widget::row(__children).spacing(::ui_lang_runtime::bounded_spacing(6.0, __child_count)).align_y(::iced::alignment::Vertical::Center); let __content = ::iced::widget::container(__layout); let __layout_content: __IceElement<'_, __DucktapeMessage> = __content.into(); let __a11y_key = format!("{}/@layout:446", __ice_use_scope); ::ui_lang_runtime::accessible(__layout_content, ::ui_lang_runtime::StableId::new(&__a11y_key), ::ui_lang_runtime::Role::GenericContainer).logical_id_maybe(::core::cfg!(test).then_some(__a11y_key)).into() };
#[cfg(test)]
drop(__ice_render_source);
__ice_rendered
}); let __child_count = __children.len(); let __children = __children.into_iter().map(|__child| ::ui_lang_runtime::bounded_fill_element(__child, __child_count, true)).collect::<::std::vec::Vec<_>>(); let __layout = ::iced::widget::row(__children).spacing(::ui_lang_runtime::bounded_spacing(13.0, __child_count)).width(::iced::Fill).height(::iced::Fill).align_y(::iced::alignment::Vertical::Center); let __content = ::iced::widget::container(__layout); let __layout_content: __IceElement<'_, __DucktapeMessage> = __content.into(); let __a11y_key = format!("{}/@layout:436", __ice_use_scope); ::ui_lang_runtime::accessible(__layout_content, ::ui_lang_runtime::StableId::new(&__a11y_key), ::ui_lang_runtime::Role::GenericContainer).logical_id_maybe(::core::cfg!(test).then_some(__a11y_key)).into() };
#[cfg(test)]
drop(__ice_render_source);
__ice_rendered
}; let __container = {  ::iced::widget::container(__container_content).padding(::ui_lang_runtime::bounded_padding(0.0, 13.0, 0.0, (13.0 + crate::backend::titlebar_inset()))).width(::iced::Fill).height(39.0 as f32).style(move |__theme| { let mut __style = ::iced::widget::container::Style::default(); __style.background = ::std::option::Option::Some(::iced::Background::Color(__ice_palette.colors[55])); __style }) }; ::ui_lang_runtime::accessible(__container, ::ui_lang_runtime::StableId::new(&__a11y_key), ::ui_lang_runtime::Role::GenericContainer).logical_id_maybe(::core::cfg!(test).then_some(__a11y_key)).into() };
#[cfg(test)]
drop(__ice_render_source);
__ice_rendered
}); __children.push({
#[cfg(test)]
let __ice_render_source_location = ::ui_lang_runtime::testing::Location::new("components/shell.ice", 517, 1, "rendered view node");
#[cfg(test)]
let __ice_render_source = ::ui_lang_runtime::testing::push_render_source(__ice_render_source_location);
let __ice_rendered: __IceElement<'_, __DucktapeMessage> = { let __a11y_key = format!("{}/@container:517", __ice_use_scope); let __container_content: __IceElement<'_, __DucktapeMessage> = {
#[cfg(test)]
let __ice_render_source_location = ::ui_lang_runtime::testing::Location::new("components/shell.ice", 522, 1, "rendered view node");
#[cfg(test)]
let __ice_render_source = ::ui_lang_runtime::testing::push_render_source(__ice_render_source_location);
let __ice_rendered: __IceElement<'_, __DucktapeMessage> = ::iced::widget::space().width(1.0 as f32).height(1.0 as f32).into();
#[cfg(test)]
drop(__ice_render_source);
__ice_rendered
}; let __container = {  ::iced::widget::container(__container_content).width(::iced::Fill).height(1.0 as f32).style(move |__theme| { let mut __style = ::iced::widget::container::Style::default(); __style.background = ::std::option::Option::Some(::iced::Background::Color(__ice_palette.colors[39])); __style }) }; ::ui_lang_runtime::accessible(__container, ::ui_lang_runtime::StableId::new(&__a11y_key), ::ui_lang_runtime::Role::GenericContainer).logical_id_maybe(::core::cfg!(test).then_some(__a11y_key)).into() };
#[cfg(test)]
drop(__ice_render_source);
__ice_rendered
}); let __child_count = __children.len(); let __children = __children.into_iter().map(|__child| ::ui_lang_runtime::bounded_fill_element(__child, __child_count, false)).collect::<::std::vec::Vec<_>>(); let __layout = ::iced::widget::column(__children).width(::iced::Fill); let __content = ::iced::widget::container(__layout); let __layout_content: __IceElement<'_, __DucktapeMessage> = __content.into(); let __a11y_key = __ice_node_scope.as_str(); ::ui_lang_runtime::accessible(__layout_content, ::ui_lang_runtime::StableId::new(&__a11y_key), ::ui_lang_runtime::Role::GenericContainer).logical_id_maybe(::core::cfg!(test).then_some(__a11y_key)).into() } };
#[cfg(test)]
drop(__ice_render_source);
__ice_rendered
}; __component_content }
pub(super) fn __ice_component_use_73(&self, __ice_palette: __IcePalette, __ice_use_scope: ::std::string::String) -> __IceElement<'_, __DucktapeMessage> { let __component_content: __IceElement<'_, __DucktapeMessage> = {
#[cfg(test)]
let __ice_render_source_location = ::ui_lang_runtime::testing::Location::new("components/shell.ice", 20, 1, "rendered view node");
#[cfg(test)]
let __ice_render_source = ::ui_lang_runtime::testing::push_render_source(__ice_render_source_location);
let __ice_rendered: __IceElement<'_, __DucktapeMessage> = { let __ice_node_scope = format!("{}/root", __ice_use_scope); { let __a11y_key = __ice_node_scope.as_str(); let __container_content: __IceElement<'_, __DucktapeMessage> = {
#[cfg(test)]
let __ice_render_source_location = ::ui_lang_runtime::testing::Location::new("components/shell.ice", 28, 1, "rendered view node");
#[cfg(test)]
let __ice_render_source = ::ui_lang_runtime::testing::push_render_source(__ice_render_source_location);
let __ice_rendered: __IceElement<'_, __DucktapeMessage> = { let __a11y_key = format!("{}/@text:28", __ice_use_scope); let __text_value = ("D").to_string(); let __text = ::iced::widget::text(__text_value.clone()).size(((14.0) as f32).max(f32::EPSILON).min(f32::MAX)).wrapping(::iced::widget::text::Wrapping::None).font(::iced::Font { family: ::iced::font::Family::Name("Geist Mono"), weight: ::iced::font::Weight::Semibold, stretch: ::iced::font::Stretch::Normal, style: ::iced::font::Style::Normal }).color(__ice_palette.colors[38]); let __text = ::ui_lang_runtime::selectable_text(__text); ::ui_lang_runtime::accessible(__text, ::ui_lang_runtime::StableId::new(&__a11y_key), ::ui_lang_runtime::Role::Label).logical_id_maybe(::core::cfg!(test).then_some(__a11y_key)).value(__text_value).into() };
#[cfg(test)]
drop(__ice_render_source);
__ice_rendered
}; let __container = {  ::iced::widget::container(__container_content).id(::iced::widget::Id::from(__ice_node_scope.clone())).width(30.0 as f32).height(30.0 as f32).align_x(::iced::alignment::Horizontal::Center).align_y(::iced::alignment::Vertical::Center).style(move |__theme| { let mut __style = ::iced::widget::container::Style::default(); __style.background = ::std::option::Option::Some(::iced::Background::Color(__ice_palette.colors[7])); __style.border.radius = ::iced::border::Radius { top_left: ((9.0) as f32).max(0.0).min(f32::MAX), top_right: ((9.0) as f32).max(0.0).min(f32::MAX), bottom_right: ((9.0) as f32).max(0.0).min(f32::MAX), bottom_left: ((9.0) as f32).max(0.0).min(f32::MAX) }; __style }) }; ::ui_lang_runtime::accessible(__container, ::ui_lang_runtime::StableId::new(&__a11y_key), ::ui_lang_runtime::Role::GenericContainer).logical_id_maybe(::core::cfg!(test).then_some(__a11y_key)).into() } };
#[cfg(test)]
drop(__ice_render_source);
__ice_rendered
}; __component_content }
pub(super) fn __ice_component_use_76(&self, __ice_palette: __IcePalette, __ice_use_scope: ::std::string::String, __ice_cb_0: impl Fn(ShellTab) -> __DucktapeMessage + Clone + 'static, __ice_arg_0: crate::backend::NavItem) -> __IceElement<'_, __DucktapeMessage> { let __component_content: __IceElement<'_, __DucktapeMessage> = {
#[cfg(test)]
let __ice_render_source_location = ::ui_lang_runtime::testing::Location::new("components/shell.ice", 572, 1, "rendered view node");
#[cfg(test)]
let __ice_render_source = ::ui_lang_runtime::testing::push_render_source(__ice_render_source_location);
let __ice_rendered: __IceElement<'_, __DucktapeMessage> = { let __ice_node_scope = format!("{}/root", __ice_use_scope); { let mut __children: ::std::vec::Vec<__IceElement<'_, __DucktapeMessage>> = ::std::vec::Vec::new(); if __ice_arg_0.active { __children.push({
#[cfg(test)]
let __ice_render_source_location = ::ui_lang_runtime::testing::Location::new("components/shell.ice", 574, 1, "rendered view node");
#[cfg(test)]
let __ice_render_source = ::ui_lang_runtime::testing::push_render_source(__ice_render_source_location);
let __ice_rendered: __IceElement<'_, __DucktapeMessage> = { let __a11y_key = format!("{}/@button:574", __ice_use_scope); let __a11y_id = ::ui_lang_runtime::StableId::new(&__a11y_key); let __disabled = false; let __activate = (__ice_cb_0)(__ice_arg_0.id.clone()); let __button_content: __IceElement<'_, __DucktapeMessage> = { let __button_inner: __IceElement<'_, __DucktapeMessage> = {
#[cfg(test)]
let __ice_render_source_location = ::ui_lang_runtime::testing::Location::new("components/shell.ice", 581, 1, "rendered view node");
#[cfg(test)]
let __ice_render_source = ::ui_lang_runtime::testing::push_render_source(__ice_render_source_location);
let __ice_rendered: __IceElement<'_, __DucktapeMessage> = { let mut __children: ::std::vec::Vec<__IceElement<'_, __DucktapeMessage>> = ::std::vec::Vec::new(); __children.push({
#[cfg(test)]
let __ice_render_source_location = ::ui_lang_runtime::testing::Location::new("components/shell.ice", 587, 1, "rendered view node");
#[cfg(test)]
let __ice_render_source = ::ui_lang_runtime::testing::push_render_source(__ice_render_source_location);
let __ice_rendered: __IceElement<'_, __DucktapeMessage> = ::ui_lang_runtime::grow_stack(|| self.__ice_component_use_74(__ice_palette, format!("{}/Icon@3507", __ice_use_scope), __ice_arg_0.icon.to_owned()));
#[cfg(test)]
drop(__ice_render_source);
__ice_rendered
}); __children.push({
#[cfg(test)]
let __ice_render_source_location = ::ui_lang_runtime::testing::Location::new("components/shell.ice", 592, 1, "rendered view node");
#[cfg(test)]
let __ice_render_source = ::ui_lang_runtime::testing::push_render_source(__ice_render_source_location);
let __ice_rendered: __IceElement<'_, __DucktapeMessage> = { let __a11y_key = format!("{}/@text:592", __ice_use_scope); let __text_value = (__ice_arg_0.title).to_string(); let __text = ::iced::widget::text(__text_value.clone()).size(((9.5) as f32).max(f32::EPSILON).min(f32::MAX)).wrapping(::iced::widget::text::Wrapping::None).font(::iced::Font { family: ::iced::font::Family::Name("Geist"), weight: ::iced::font::Weight::Semibold, stretch: ::iced::font::Stretch::Normal, style: ::iced::font::Style::Normal }).color(__ice_palette.colors[69]); let __text = ::ui_lang_runtime::selectable_text(__text); ::ui_lang_runtime::accessible(__text, ::ui_lang_runtime::StableId::new(&__a11y_key), ::ui_lang_runtime::Role::Label).logical_id_maybe(::core::cfg!(test).then_some(__a11y_key)).value(__text_value).into() };
#[cfg(test)]
drop(__ice_render_source);
__ice_rendered
}); let __child_count = __children.len(); let __children = __children.into_iter().map(|__child| ::ui_lang_runtime::bounded_fill_element(__child, __child_count, false)).collect::<::std::vec::Vec<_>>(); let __layout = ::iced::widget::column(__children).spacing(::ui_lang_runtime::bounded_spacing(4.0, __child_count)).padding(::ui_lang_runtime::bounded_padding(4.0, 0.0, 4.0, 0.0)).width(::iced::Fill).align_x(::iced::alignment::Horizontal::Center); let __content = ::iced::widget::container(__layout); let __layout_content: __IceElement<'_, __DucktapeMessage> = __content.into(); let __a11y_key = format!("{}/@layout:581", __ice_use_scope); ::ui_lang_runtime::accessible(__layout_content, ::ui_lang_runtime::StableId::new(&__a11y_key), ::ui_lang_runtime::Role::GenericContainer).logical_id_maybe(::core::cfg!(test).then_some(__a11y_key)).into() };
#[cfg(test)]
drop(__ice_render_source);
__ice_rendered
}; ::iced::widget::container(__button_inner).width(::iced::Fill).align_x(::iced::alignment::Horizontal::Center).into() }; let __button = ::iced::widget::button(__button_content).width(58.0 as f32).padding(0.0 as f32).on_press_maybe(if __disabled { None } else { Some(__activate.clone()) }).style(move |__theme, __status| { let mut __style = ::iced::widget::button::primary(__theme, __status); let __background: Option<::iced::Color> = match __status { ::iced::widget::button::Status::Hovered => Some(__ice_palette.colors[14]), ::iced::widget::button::Status::Pressed => Some(__ice_palette.colors[39]), ::iced::widget::button::Status::Disabled => Some(::iced::Color::from_rgba8(0, 0, 0, 0.000000)), _ => Some(::iced::Color::from_rgba8(0, 0, 0, 0.000000)) }; if let Some(__background) = __background { __style.background = Some(::iced::Background::Color(__background)); } __style.text_color = __ice_palette.colors[4]; __style.border.radius = 7.0.into(); __style.background = ::std::option::Option::Some(::iced::Background::Color(__ice_palette.colors[91])); __style.text_color = __ice_palette.colors[4]; __style.border.color = ::iced::Color::from_rgba8(0, 0, 0, 0.000000); __style.border.width = 1.0 as f32; __style.border.radius = ::iced::border::Radius { top_left: ((10.0) as f32).max(0.0).min(f32::MAX), top_right: ((10.0) as f32).max(0.0).min(f32::MAX), bottom_right: ((10.0) as f32).max(0.0).min(f32::MAX), bottom_left: ((10.0) as f32).max(0.0).min(f32::MAX) }; match __status { ::iced::widget::button::Status::Hovered => { __style.background = ::std::option::Option::Some(::iced::Background::Color(__ice_palette.colors[58])); __style.text_color = __ice_palette.colors[4]; } ::iced::widget::button::Status::Pressed => { __style.background = ::std::option::Option::Some(::iced::Background::Color(__ice_palette.colors[91])); __style.text_color = __ice_palette.colors[4]; } _ => {} } if matches!(__status, ::iced::widget::button::Status::Disabled) { if let Some(::iced::Background::Color(mut __color)) = __style.background { __color.a *= 0.5; __style.background = Some(::iced::Background::Color(__color)); } __style.text_color.a *= 0.5; } __style }); ::ui_lang_runtime::accessible(__button, __a11y_id, ::ui_lang_runtime::Role::Button).logical_id_maybe(::core::cfg!(test).then_some(&*__a11y_key)).focus_id(::iced::widget::Id::from(__a11y_key)).label(__ice_arg_0.title.to_owned()).checked(__ice_arg_0.active).disabled(__disabled).on_activate_maybe(if __disabled { None } else { Some(__activate) }).focus_ring(__ice_palette.colors[42], 7.0).into() };
#[cfg(test)]
drop(__ice_render_source);
__ice_rendered
}); } if (!__ice_arg_0.active) { __children.push({
#[cfg(test)]
let __ice_render_source_location = ::ui_lang_runtime::testing::Location::new("components/shell.ice", 602, 1, "rendered view node");
#[cfg(test)]
let __ice_render_source = ::ui_lang_runtime::testing::push_render_source(__ice_render_source_location);
let __ice_rendered: __IceElement<'_, __DucktapeMessage> = { let __a11y_key = format!("{}/@button:602", __ice_use_scope); let __a11y_id = ::ui_lang_runtime::StableId::new(&__a11y_key); let __disabled = false; let __activate = (__ice_cb_0)(__ice_arg_0.id.clone()); let __button_content: __IceElement<'_, __DucktapeMessage> = { let __button_inner: __IceElement<'_, __DucktapeMessage> = {
#[cfg(test)]
let __ice_render_source_location = ::ui_lang_runtime::testing::Location::new("components/shell.ice", 609, 1, "rendered view node");
#[cfg(test)]
let __ice_render_source = ::ui_lang_runtime::testing::push_render_source(__ice_render_source_location);
let __ice_rendered: __IceElement<'_, __DucktapeMessage> = { let mut __children: ::std::vec::Vec<__IceElement<'_, __DucktapeMessage>> = ::std::vec::Vec::new(); __children.push({
#[cfg(test)]
let __ice_render_source_location = ::ui_lang_runtime::testing::Location::new("components/shell.ice", 615, 1, "rendered view node");
#[cfg(test)]
let __ice_render_source = ::ui_lang_runtime::testing::push_render_source(__ice_render_source_location);
let __ice_rendered: __IceElement<'_, __DucktapeMessage> = ::ui_lang_runtime::grow_stack(|| self.__ice_component_use_75(__ice_palette, format!("{}/Icon@3535", __ice_use_scope), __ice_arg_0.icon.to_owned()));
#[cfg(test)]
drop(__ice_render_source);
__ice_rendered
}); __children.push({
#[cfg(test)]
let __ice_render_source_location = ::ui_lang_runtime::testing::Location::new("components/shell.ice", 620, 1, "rendered view node");
#[cfg(test)]
let __ice_render_source = ::ui_lang_runtime::testing::push_render_source(__ice_render_source_location);
let __ice_rendered: __IceElement<'_, __DucktapeMessage> = { let __a11y_key = format!("{}/@text:620", __ice_use_scope); let __text_value = (__ice_arg_0.title).to_string(); let __text = ::iced::widget::text(__text_value.clone()).size(((9.5) as f32).max(f32::EPSILON).min(f32::MAX)).wrapping(::iced::widget::text::Wrapping::None).font(::iced::Font { family: ::iced::font::Family::Name("Geist"), weight: ::iced::font::Weight::Semibold, stretch: ::iced::font::Stretch::Normal, style: ::iced::font::Style::Normal }).color(__ice_palette.colors[70]); let __text = ::ui_lang_runtime::selectable_text(__text); ::ui_lang_runtime::accessible(__text, ::ui_lang_runtime::StableId::new(&__a11y_key), ::ui_lang_runtime::Role::Label).logical_id_maybe(::core::cfg!(test).then_some(__a11y_key)).value(__text_value).into() };
#[cfg(test)]
drop(__ice_render_source);
__ice_rendered
}); let __child_count = __children.len(); let __children = __children.into_iter().map(|__child| ::ui_lang_runtime::bounded_fill_element(__child, __child_count, false)).collect::<::std::vec::Vec<_>>(); let __layout = ::iced::widget::column(__children).spacing(::ui_lang_runtime::bounded_spacing(4.0, __child_count)).padding(::ui_lang_runtime::bounded_padding(4.0, 0.0, 4.0, 0.0)).width(::iced::Fill).align_x(::iced::alignment::Horizontal::Center); let __content = ::iced::widget::container(__layout); let __layout_content: __IceElement<'_, __DucktapeMessage> = __content.into(); let __a11y_key = format!("{}/@layout:609", __ice_use_scope); ::ui_lang_runtime::accessible(__layout_content, ::ui_lang_runtime::StableId::new(&__a11y_key), ::ui_lang_runtime::Role::GenericContainer).logical_id_maybe(::core::cfg!(test).then_some(__a11y_key)).into() };
#[cfg(test)]
drop(__ice_render_source);
__ice_rendered
}; ::iced::widget::container(__button_inner).width(::iced::Fill).align_x(::iced::alignment::Horizontal::Center).into() }; let __button = ::iced::widget::button(__button_content).width(58.0 as f32).padding(0.0 as f32).on_press_maybe(if __disabled { None } else { Some(__activate.clone()) }).style(move |__theme, __status| { let mut __style = ::iced::widget::button::primary(__theme, __status); let __background: Option<::iced::Color> = match __status { ::iced::widget::button::Status::Hovered => Some(__ice_palette.colors[14]), ::iced::widget::button::Status::Pressed => Some(__ice_palette.colors[39]), ::iced::widget::button::Status::Disabled => Some(::iced::Color::from_rgba8(0, 0, 0, 0.000000)), _ => Some(::iced::Color::from_rgba8(0, 0, 0, 0.000000)) }; if let Some(__background) = __background { __style.background = Some(::iced::Background::Color(__background)); } __style.text_color = __ice_palette.colors[4]; __style.border.radius = 7.0.into(); __style.background = ::std::option::Option::Some(::iced::Background::Color(::iced::Color::from_rgba8(0, 0, 0, 0.000000))); __style.text_color = __ice_palette.colors[5]; __style.border.color = ::iced::Color::from_rgba8(0, 0, 0, 0.000000); __style.border.width = 1.0 as f32; __style.border.radius = ::iced::border::Radius { top_left: ((10.0) as f32).max(0.0).min(f32::MAX), top_right: ((10.0) as f32).max(0.0).min(f32::MAX), bottom_right: ((10.0) as f32).max(0.0).min(f32::MAX), bottom_left: ((10.0) as f32).max(0.0).min(f32::MAX) }; match __status { ::iced::widget::button::Status::Hovered => { __style.background = ::std::option::Option::Some(::iced::Background::Color(__ice_palette.colors[58])); __style.text_color = __ice_palette.colors[4]; } ::iced::widget::button::Status::Pressed => { __style.background = ::std::option::Option::Some(::iced::Background::Color(__ice_palette.colors[56])); __style.text_color = __ice_palette.colors[4]; } _ => {} } if matches!(__status, ::iced::widget::button::Status::Disabled) { if let Some(::iced::Background::Color(mut __color)) = __style.background { __color.a *= 0.5; __style.background = Some(::iced::Background::Color(__color)); } __style.text_color.a *= 0.5; } __style }); ::ui_lang_runtime::accessible(__button, __a11y_id, ::ui_lang_runtime::Role::Button).logical_id_maybe(::core::cfg!(test).then_some(&*__a11y_key)).focus_id(::iced::widget::Id::from(__a11y_key)).label(__ice_arg_0.title.to_owned()).checked(__ice_arg_0.active).disabled(__disabled).on_activate_maybe(if __disabled { None } else { Some(__activate) }).focus_ring(__ice_palette.colors[42], 7.0).into() };
#[cfg(test)]
drop(__ice_render_source);
__ice_rendered
}); } if __ice_arg_0.live { __children.push({
#[cfg(test)]
let __ice_render_source_location = ::ui_lang_runtime::testing::Location::new("components/shell.ice", 630, 1, "rendered view node");
#[cfg(test)]
let __ice_render_source = ::ui_lang_runtime::testing::push_render_source(__ice_render_source_location);
let __ice_rendered: __IceElement<'_, __DucktapeMessage> = { let __pin_content: __IceElement<'_, __DucktapeMessage> = {
#[cfg(test)]
let __ice_render_source_location = ::ui_lang_runtime::testing::Location::new("components/shell.ice", 631, 1, "rendered view node");
#[cfg(test)]
let __ice_render_source = ::ui_lang_runtime::testing::push_render_source(__ice_render_source_location);
let __ice_rendered: __IceElement<'_, __DucktapeMessage> = { let __a11y_key = format!("{}/@container:631", __ice_use_scope); let __container_content: __IceElement<'_, __DucktapeMessage> = {
#[cfg(test)]
let __ice_render_source_location = ::ui_lang_runtime::testing::Location::new("components/shell.ice", 636, 1, "rendered view node");
#[cfg(test)]
let __ice_render_source = ::ui_lang_runtime::testing::push_render_source(__ice_render_source_location);
let __ice_rendered: __IceElement<'_, __DucktapeMessage> = { let __ice_component_050756c7365446f74_scope_3556 = format!("{}/PulseDot@3556", __ice_use_scope); ::ui_lang_runtime::rev_memo(420u64, (__ice_palette.name), ::ui_lang_runtime::grow_stack(|| self.__ice_component_use_42(__ice_palette, __ice_component_050756c7365446f74_scope_3556))).into() };
#[cfg(test)]
drop(__ice_render_source);
__ice_rendered
}; let __container = {  ::iced::widget::container(__container_content).padding(::ui_lang_runtime::bounded_padding(1.5, 1.5, 1.5, 1.5)).style(move |__theme| { let mut __style = ::iced::widget::container::Style::default(); __style.background = ::std::option::Option::Some(::iced::Background::Color(__ice_palette.colors[53])); __style.border.radius = ::iced::border::Radius { top_left: ((5.0) as f32).max(0.0).min(f32::MAX), top_right: ((5.0) as f32).max(0.0).min(f32::MAX), bottom_right: ((5.0) as f32).max(0.0).min(f32::MAX), bottom_left: ((5.0) as f32).max(0.0).min(f32::MAX) }; __style }) }; ::ui_lang_runtime::accessible(__container, ::ui_lang_runtime::StableId::new(&__a11y_key), ::ui_lang_runtime::Role::GenericContainer).logical_id_maybe(::core::cfg!(test).then_some(__a11y_key)).into() };
#[cfg(test)]
drop(__ice_render_source);
__ice_rendered
}; ::iced::widget::pin(__pin_content).x(36.0 as f32).y(6.0 as f32).into() };
#[cfg(test)]
drop(__ice_render_source);
__ice_rendered
}); } if ((__ice_arg_0.badge > 0) && (__ice_arg_0.badge < 10)) { __children.push({
#[cfg(test)]
let __ice_render_source_location = ::ui_lang_runtime::testing::Location::new("components/shell.ice", 640, 1, "rendered view node");
#[cfg(test)]
let __ice_render_source = ::ui_lang_runtime::testing::push_render_source(__ice_render_source_location);
let __ice_rendered: __IceElement<'_, __DucktapeMessage> = { let __pin_content: __IceElement<'_, __DucktapeMessage> = {
#[cfg(test)]
let __ice_render_source_location = ::ui_lang_runtime::testing::Location::new("components/shell.ice", 641, 1, "rendered view node");
#[cfg(test)]
let __ice_render_source = ::ui_lang_runtime::testing::push_render_source(__ice_render_source_location);
let __ice_rendered: __IceElement<'_, __DucktapeMessage> = { let __a11y_key = format!("{}/@container:641", __ice_use_scope); let __container_content: __IceElement<'_, __DucktapeMessage> = {
#[cfg(test)]
let __ice_render_source_location = ::ui_lang_runtime::testing::Location::new("components/shell.ice", 651, 1, "rendered view node");
#[cfg(test)]
let __ice_render_source = ::ui_lang_runtime::testing::push_render_source(__ice_render_source_location);
let __ice_rendered: __IceElement<'_, __DucktapeMessage> = { let __a11y_key = format!("{}/@text:651", __ice_use_scope); let __text_value = (__ice_arg_0.badge).to_string(); let __text = ::iced::widget::text(__text_value.clone()).size(((9.0) as f32).max(f32::EPSILON).min(f32::MAX)).wrapping(::iced::widget::text::Wrapping::None).font(::iced::Font { family: ::iced::font::Family::Name("Geist Mono"), weight: ::iced::font::Weight::Semibold, stretch: ::iced::font::Stretch::Normal, style: ::iced::font::Style::Normal }).color(__ice_palette.colors[17]); let __text = ::ui_lang_runtime::selectable_text(__text); ::ui_lang_runtime::accessible(__text, ::ui_lang_runtime::StableId::new(&__a11y_key), ::ui_lang_runtime::Role::Label).logical_id_maybe(::core::cfg!(test).then_some(__a11y_key)).value(__text_value).into() };
#[cfg(test)]
drop(__ice_render_source);
__ice_rendered
}; let __container = {  ::iced::widget::container(__container_content).width(15.0 as f32).height(15.0 as f32).align_x(::iced::alignment::Horizontal::Center).align_y(::iced::alignment::Vertical::Center).style(move |__theme| { let mut __style = ::iced::widget::container::Style::default(); __style.background = ::std::option::Option::Some(::iced::Background::Color(__ice_palette.colors[16])); __style.border.color = __ice_palette.colors[53]; __style.border.width = 1.5 as f32; __style.border.radius = ::iced::border::Radius { top_left: ((8.0) as f32).max(0.0).min(f32::MAX), top_right: ((8.0) as f32).max(0.0).min(f32::MAX), bottom_right: ((8.0) as f32).max(0.0).min(f32::MAX), bottom_left: ((8.0) as f32).max(0.0).min(f32::MAX) }; __style }) }; ::ui_lang_runtime::accessible(__container, ::ui_lang_runtime::StableId::new(&__a11y_key), ::ui_lang_runtime::Role::GenericContainer).logical_id_maybe(::core::cfg!(test).then_some(__a11y_key)).into() };
#[cfg(test)]
drop(__ice_render_source);
__ice_rendered
}; ::iced::widget::pin(__pin_content).x(32.0 as f32).y(6.0 as f32).into() };
#[cfg(test)]
drop(__ice_render_source);
__ice_rendered
}); } if (__ice_arg_0.badge > 9) { __children.push({
#[cfg(test)]
let __ice_render_source_location = ::ui_lang_runtime::testing::Location::new("components/shell.ice", 658, 1, "rendered view node");
#[cfg(test)]
let __ice_render_source = ::ui_lang_runtime::testing::push_render_source(__ice_render_source_location);
let __ice_rendered: __IceElement<'_, __DucktapeMessage> = { let __pin_content: __IceElement<'_, __DucktapeMessage> = {
#[cfg(test)]
let __ice_render_source_location = ::ui_lang_runtime::testing::Location::new("components/shell.ice", 659, 1, "rendered view node");
#[cfg(test)]
let __ice_render_source = ::ui_lang_runtime::testing::push_render_source(__ice_render_source_location);
let __ice_rendered: __IceElement<'_, __DucktapeMessage> = { let __a11y_key = format!("{}/@container:659", __ice_use_scope); let __container_content: __IceElement<'_, __DucktapeMessage> = {
#[cfg(test)]
let __ice_render_source_location = ::ui_lang_runtime::testing::Location::new("components/shell.ice", 669, 1, "rendered view node");
#[cfg(test)]
let __ice_render_source = ::ui_lang_runtime::testing::push_render_source(__ice_render_source_location);
let __ice_rendered: __IceElement<'_, __DucktapeMessage> = { let __a11y_key = format!("{}/@text:669", __ice_use_scope); let __text_value = (__ice_arg_0.badge).to_string(); let __text = ::iced::widget::text(__text_value.clone()).size(((9.0) as f32).max(f32::EPSILON).min(f32::MAX)).wrapping(::iced::widget::text::Wrapping::None).font(::iced::Font { family: ::iced::font::Family::Name("Geist Mono"), weight: ::iced::font::Weight::Semibold, stretch: ::iced::font::Stretch::Normal, style: ::iced::font::Style::Normal }).color(__ice_palette.colors[17]); let __text = ::ui_lang_runtime::selectable_text(__text); ::ui_lang_runtime::accessible(__text, ::ui_lang_runtime::StableId::new(&__a11y_key), ::ui_lang_runtime::Role::Label).logical_id_maybe(::core::cfg!(test).then_some(__a11y_key)).value(__text_value).into() };
#[cfg(test)]
drop(__ice_render_source);
__ice_rendered
}; let __container = {  ::iced::widget::container(__container_content).width(22.0 as f32).height(15.0 as f32).align_x(::iced::alignment::Horizontal::Center).align_y(::iced::alignment::Vertical::Center).style(move |__theme| { let mut __style = ::iced::widget::container::Style::default(); __style.background = ::std::option::Option::Some(::iced::Background::Color(__ice_palette.colors[16])); __style.border.color = __ice_palette.colors[53]; __style.border.width = 1.5 as f32; __style.border.radius = ::iced::border::Radius { top_left: ((8.0) as f32).max(0.0).min(f32::MAX), top_right: ((8.0) as f32).max(0.0).min(f32::MAX), bottom_right: ((8.0) as f32).max(0.0).min(f32::MAX), bottom_left: ((8.0) as f32).max(0.0).min(f32::MAX) }; __style }) }; ::ui_lang_runtime::accessible(__container, ::ui_lang_runtime::StableId::new(&__a11y_key), ::ui_lang_runtime::Role::GenericContainer).logical_id_maybe(::core::cfg!(test).then_some(__a11y_key)).into() };
#[cfg(test)]
drop(__ice_render_source);
__ice_rendered
}; ::iced::widget::pin(__pin_content).x(25.0 as f32).y(6.0 as f32).into() };
#[cfg(test)]
drop(__ice_render_source);
__ice_rendered
}); } let __layout = ::ui_lang_runtime::zstack(__children).width(58.0 as f32); let __content = ::iced::widget::container(__layout); let __layout_content: __IceElement<'_, __DucktapeMessage> = __content.into(); let __a11y_key = __ice_node_scope.as_str(); ::ui_lang_runtime::accessible(__layout_content, ::ui_lang_runtime::StableId::new(&__a11y_key), ::ui_lang_runtime::Role::GenericContainer).logical_id_maybe(::core::cfg!(test).then_some(__a11y_key)).into() } };
#[cfg(test)]
drop(__ice_render_source);
__ice_rendered
}; __component_content }
pub(super) fn __ice_component_use_82(&self, __ice_palette: __IcePalette, __ice_use_scope: ::std::string::String, __ice_cb_0: impl Fn(ShellTab) -> __DucktapeMessage + Clone + 'static) -> __IceElement<'_, __DucktapeMessage> { let __component_content: __IceElement<'_, __DucktapeMessage> = {
#[cfg(test)]
let __ice_render_source_location = ::ui_lang_runtime::testing::Location::new("components/shell.ice", 679, 1, "rendered view node");
#[cfg(test)]
let __ice_render_source = ::ui_lang_runtime::testing::push_render_source(__ice_render_source_location);
let __ice_rendered: __IceElement<'_, __DucktapeMessage> = { let __ice_node_scope = format!("{}/root", __ice_use_scope); { let __a11y_key = __ice_node_scope.as_str(); let __container_content: __IceElement<'_, __DucktapeMessage> = {
#[cfg(test)]
let __ice_render_source_location = ::ui_lang_runtime::testing::Location::new("components/shell.ice", 687, 1, "rendered view node");
#[cfg(test)]
let __ice_render_source = ::ui_lang_runtime::testing::push_render_source(__ice_render_source_location);
let __ice_rendered: __IceElement<'_, __DucktapeMessage> = { let mut __children: ::std::vec::Vec<__IceElement<'_, __DucktapeMessage>> = ::std::vec::Vec::new(); __children.push({
#[cfg(test)]
let __ice_render_source_location = ::ui_lang_runtime::testing::Location::new("components/shell.ice", 693, 1, "rendered view node");
#[cfg(test)]
let __ice_render_source = ::ui_lang_runtime::testing::push_render_source(__ice_render_source_location);
let __ice_rendered: __IceElement<'_, __DucktapeMessage> = { let __ice_component_brand_scope_3613 = format!("{}/Brand@3613", __ice_use_scope); ::ui_lang_runtime::rev_memo(431u64, (__ice_palette.name), ::ui_lang_runtime::grow_stack(|| self.__ice_component_use_73(__ice_palette, __ice_component_brand_scope_3613))).into() };
#[cfg(test)]
drop(__ice_render_source);
__ice_rendered
}); __children.push({
#[cfg(test)]
let __ice_render_source_location = ::ui_lang_runtime::testing::Location::new("components/shell.ice", 694, 1, "rendered view node");
#[cfg(test)]
let __ice_render_source = ::ui_lang_runtime::testing::push_render_source(__ice_render_source_location);
let __ice_rendered: __IceElement<'_, __DucktapeMessage> = ::iced::widget::space().width(1.0 as f32).height(9.0 as f32).into();
#[cfg(test)]
drop(__ice_render_source);
__ice_rendered
}); __children.push({
#[cfg(test)]
let __ice_render_source_location = ::ui_lang_runtime::testing::Location::new("components/shell.ice", 695, 1, "rendered view node");
#[cfg(test)]
let __ice_render_source = ::ui_lang_runtime::testing::push_render_source(__ice_render_source_location);
let __ice_rendered: __IceElement<'_, __DucktapeMessage> = { let __a11y_key = format!("{}/@layout:695", __ice_use_scope); let __scroll_content: __IceElement<'_, __DucktapeMessage> = {
#[cfg(test)]
let __ice_render_source_location = ::ui_lang_runtime::testing::Location::new("components/shell.ice", 701, 1, "rendered view node");
#[cfg(test)]
let __ice_render_source = ::ui_lang_runtime::testing::push_render_source(__ice_render_source_location);
let __ice_rendered: __IceElement<'_, __DucktapeMessage> = { let mut __children: ::std::vec::Vec<__IceElement<'_, __DucktapeMessage>> = ::std::vec::Vec::new(); for (__ice_index, item) in crate::backend::shell_nav(self.shell_tab.clone(), self.gov_open, self.agents_live).iter().enumerate() { let __for_scope = format!("{}/@for:3626({})", __ice_use_scope, __ice_index); __children.push({
#[cfg(test)]
let __ice_render_source_location = ::ui_lang_runtime::testing::Location::new("components/shell.ice", 707, 1, "rendered view node");
#[cfg(test)]
let __ice_render_source = ::ui_lang_runtime::testing::push_render_source(__ice_render_source_location);
let __ice_rendered: __IceElement<'_, __DucktapeMessage> = ::ui_lang_runtime::grow_stack(|| self.__ice_component_use_76(__ice_palette, format!("{}/RailButton@3627", __for_scope), (__ice_cb_0).clone(), item.clone()));
#[cfg(test)]
drop(__ice_render_source);
__ice_rendered
}); } let __child_count = __children.len(); let __children = __children.into_iter().map(|__child| ::ui_lang_runtime::bounded_fill_element(__child, __child_count, false)).collect::<::std::vec::Vec<_>>(); let __layout = ::iced::widget::column(__children).spacing(::ui_lang_runtime::bounded_spacing(2.0, __child_count)).width(::iced::Fill).align_x(::iced::alignment::Horizontal::Center); let __content = ::iced::widget::container(__layout); let __layout_content: __IceElement<'_, __DucktapeMessage> = __content.into(); let __a11y_key = format!("{}/@layout:701", __ice_use_scope); ::ui_lang_runtime::accessible(__layout_content, ::ui_lang_runtime::StableId::new(&__a11y_key), ::ui_lang_runtime::Role::GenericContainer).logical_id_maybe(::core::cfg!(test).then_some(__a11y_key)).into() };
#[cfg(test)]
drop(__ice_render_source);
__ice_rendered
}; let __layout = ::iced::widget::scrollable(__scroll_content).direction(::iced::widget::scrollable::Direction::Vertical(::iced::widget::scrollable::Scrollbar::hidden())).anchor_x(::iced::widget::scrollable::Anchor::Start).anchor_y(::iced::widget::scrollable::Anchor::Start).width(::iced::Fill).height(::iced::Fill); ::ui_lang_runtime::accessible(__layout, ::ui_lang_runtime::StableId::new(&__a11y_key), ::ui_lang_runtime::Role::GenericContainer).logical_id_maybe(::core::cfg!(test).then_some(__a11y_key)).into() };
#[cfg(test)]
drop(__ice_render_source);
__ice_rendered
}); if (self.shell_tab == ShellTab::Settings) { __children.push({
#[cfg(test)]
let __ice_render_source_location = ::ui_lang_runtime::testing::Location::new("components/shell.ice", 711, 1, "rendered view node");
#[cfg(test)]
let __ice_render_source = ::ui_lang_runtime::testing::push_render_source(__ice_render_source_location);
let __ice_rendered: __IceElement<'_, __DucktapeMessage> = { let __a11y_key = format!("{}/@button:711", __ice_use_scope); let __a11y_id = ::ui_lang_runtime::StableId::new(&__a11y_key); let __disabled = false; let __activate = (__ice_cb_0)(ShellTab::Settings); let __button_content: __IceElement<'_, __DucktapeMessage> = {
#[cfg(test)]
let __ice_render_source_location = ::ui_lang_runtime::testing::Location::new("components/shell.ice", 717, 1, "rendered view node");
#[cfg(test)]
let __ice_render_source = ::ui_lang_runtime::testing::push_render_source(__ice_render_source_location);
let __ice_rendered: __IceElement<'_, __DucktapeMessage> = ::ui_lang_runtime::grow_stack(|| self.__ice_component_use_77(__ice_palette, format!("{}/Icon@3637", __ice_use_scope)));
#[cfg(test)]
drop(__ice_render_source);
__ice_rendered
}; let __button = ::iced::widget::button(__button_content).padding(8.0 as f32).on_press_maybe(if __disabled { None } else { Some(__activate.clone()) }).style(move |__theme, __status| { let mut __style = ::iced::widget::button::primary(__theme, __status); let __background: Option<::iced::Color> = match __status { ::iced::widget::button::Status::Hovered => Some(__ice_palette.colors[14]), ::iced::widget::button::Status::Pressed => Some(__ice_palette.colors[39]), ::iced::widget::button::Status::Disabled => Some(::iced::Color::from_rgba8(0, 0, 0, 0.000000)), _ => Some(::iced::Color::from_rgba8(0, 0, 0, 0.000000)) }; if let Some(__background) = __background { __style.background = Some(::iced::Background::Color(__background)); } __style.text_color = __ice_palette.colors[4]; __style.border.radius = 7.0.into(); __style.background = ::std::option::Option::Some(::iced::Background::Color(__ice_palette.colors[91])); __style.text_color = __ice_palette.colors[4]; __style.border.color = ::iced::Color::from_rgba8(0, 0, 0, 0.000000); __style.border.width = 1.0 as f32; __style.border.radius = ::iced::border::Radius { top_left: ((9.0) as f32).max(0.0).min(f32::MAX), top_right: ((9.0) as f32).max(0.0).min(f32::MAX), bottom_right: ((9.0) as f32).max(0.0).min(f32::MAX), bottom_left: ((9.0) as f32).max(0.0).min(f32::MAX) }; match __status { ::iced::widget::button::Status::Hovered => { __style.background = ::std::option::Option::Some(::iced::Background::Color(__ice_palette.colors[58])); __style.text_color = __ice_palette.colors[4]; } ::iced::widget::button::Status::Pressed => { __style.background = ::std::option::Option::Some(::iced::Background::Color(__ice_palette.colors[91])); __style.text_color = __ice_palette.colors[4]; } _ => {} } if matches!(__status, ::iced::widget::button::Status::Disabled) { if let Some(::iced::Background::Color(mut __color)) = __style.background { __color.a *= 0.5; __style.background = Some(::iced::Background::Color(__color)); } __style.text_color.a *= 0.5; } __style }); ::ui_lang_runtime::accessible(__button, __a11y_id, ::ui_lang_runtime::Role::Button).logical_id_maybe(::core::cfg!(test).then_some(&*__a11y_key)).focus_id(::iced::widget::Id::from(__a11y_key)).label("Settings".to_owned()).checked(true).disabled(__disabled).on_activate_maybe(if __disabled { None } else { Some(__activate) }).focus_ring(__ice_palette.colors[42], 7.0).into() };
#[cfg(test)]
drop(__ice_render_source);
__ice_rendered
}); } if (self.shell_tab != ShellTab::Settings) { __children.push({
#[cfg(test)]
let __ice_render_source_location = ::ui_lang_runtime::testing::Location::new("components/shell.ice", 726, 1, "rendered view node");
#[cfg(test)]
let __ice_render_source = ::ui_lang_runtime::testing::push_render_source(__ice_render_source_location);
let __ice_rendered: __IceElement<'_, __DucktapeMessage> = { let __a11y_key = format!("{}/@button:726", __ice_use_scope); let __a11y_id = ::ui_lang_runtime::StableId::new(&__a11y_key); let __disabled = false; let __activate = (__ice_cb_0)(ShellTab::Settings); let __button_content: __IceElement<'_, __DucktapeMessage> = {
#[cfg(test)]
let __ice_render_source_location = ::ui_lang_runtime::testing::Location::new("components/shell.ice", 732, 1, "rendered view node");
#[cfg(test)]
let __ice_render_source = ::ui_lang_runtime::testing::push_render_source(__ice_render_source_location);
let __ice_rendered: __IceElement<'_, __DucktapeMessage> = ::ui_lang_runtime::grow_stack(|| self.__ice_component_use_78(__ice_palette, format!("{}/Icon@3652", __ice_use_scope)));
#[cfg(test)]
drop(__ice_render_source);
__ice_rendered
}; let __button = ::iced::widget::button(__button_content).padding(8.0 as f32).on_press_maybe(if __disabled { None } else { Some(__activate.clone()) }).style(move |__theme, __status| { let mut __style = ::iced::widget::button::primary(__theme, __status); let __background: Option<::iced::Color> = match __status { ::iced::widget::button::Status::Hovered => Some(__ice_palette.colors[14]), ::iced::widget::button::Status::Pressed => Some(__ice_palette.colors[39]), ::iced::widget::button::Status::Disabled => Some(::iced::Color::from_rgba8(0, 0, 0, 0.000000)), _ => Some(::iced::Color::from_rgba8(0, 0, 0, 0.000000)) }; if let Some(__background) = __background { __style.background = Some(::iced::Background::Color(__background)); } __style.text_color = __ice_palette.colors[4]; __style.border.radius = 7.0.into(); __style.background = ::std::option::Option::Some(::iced::Background::Color(::iced::Color::from_rgba8(0, 0, 0, 0.000000))); __style.text_color = __ice_palette.colors[5]; __style.border.color = ::iced::Color::from_rgba8(0, 0, 0, 0.000000); __style.border.width = 1.0 as f32; __style.border.radius = ::iced::border::Radius { top_left: ((9.0) as f32).max(0.0).min(f32::MAX), top_right: ((9.0) as f32).max(0.0).min(f32::MAX), bottom_right: ((9.0) as f32).max(0.0).min(f32::MAX), bottom_left: ((9.0) as f32).max(0.0).min(f32::MAX) }; match __status { ::iced::widget::button::Status::Hovered => { __style.background = ::std::option::Option::Some(::iced::Background::Color(__ice_palette.colors[58])); __style.text_color = __ice_palette.colors[4]; } ::iced::widget::button::Status::Pressed => { __style.background = ::std::option::Option::Some(::iced::Background::Color(__ice_palette.colors[56])); __style.text_color = __ice_palette.colors[4]; } _ => {} } if matches!(__status, ::iced::widget::button::Status::Disabled) { if let Some(::iced::Background::Color(mut __color)) = __style.background { __color.a *= 0.5; __style.background = Some(::iced::Background::Color(__color)); } __style.text_color.a *= 0.5; } __style }); ::ui_lang_runtime::accessible(__button, __a11y_id, ::ui_lang_runtime::Role::Button).logical_id_maybe(::core::cfg!(test).then_some(&*__a11y_key)).focus_id(::iced::widget::Id::from(__a11y_key)).label("Settings".to_owned()).checked(false).disabled(__disabled).on_activate_maybe(if __disabled { None } else { Some(__activate) }).focus_ring(__ice_palette.colors[42], 7.0).into() };
#[cfg(test)]
drop(__ice_render_source);
__ice_rendered
}); } __children.push({
#[cfg(test)]
let __ice_render_source_location = ::ui_lang_runtime::testing::Location::new("components/shell.ice", 740, 1, "rendered view node");
#[cfg(test)]
let __ice_render_source = ::ui_lang_runtime::testing::push_render_source(__ice_render_source_location);
let __ice_rendered: __IceElement<'_, __DucktapeMessage> = ::iced::widget::space().width(1.0 as f32).height(6.0 as f32).into();
#[cfg(test)]
drop(__ice_render_source);
__ice_rendered
}); __children.push({
#[cfg(test)]
let __ice_render_source_location = ::ui_lang_runtime::testing::Location::new("components/shell.ice", 744, 1, "rendered view node");
#[cfg(test)]
let __ice_render_source = ::ui_lang_runtime::testing::push_render_source(__ice_render_source_location);
let __ice_rendered: __IceElement<'_, __DucktapeMessage> = { let __a11y_key = format!("{}/@button:744", __ice_use_scope); let __a11y_id = ::ui_lang_runtime::StableId::new(&__a11y_key); let __disabled = false; let __activate = (__ice_cb_0)(ShellTab::Settings); let __button_content: __IceElement<'_, __DucktapeMessage> = {
#[cfg(test)]
let __ice_render_source_location = ::ui_lang_runtime::testing::Location::new("components/shell.ice", 749, 1, "rendered view node");
#[cfg(test)]
let __ice_render_source = ::ui_lang_runtime::testing::push_render_source(__ice_render_source_location);
let __ice_rendered: __IceElement<'_, __DucktapeMessage> = { let __a11y_key = format!("{}/@container:749", __ice_use_scope); let __container_content: __IceElement<'_, __DucktapeMessage> = {
#[cfg(test)]
let __ice_render_source_location = ::ui_lang_runtime::testing::Location::new("components/shell.ice", 754, 1, "rendered view node");
#[cfg(test)]
let __ice_render_source = ::ui_lang_runtime::testing::push_render_source(__ice_render_source_location);
let __ice_rendered: __IceElement<'_, __DucktapeMessage> = { let __ice_component_05072696e636970616c417661746172_scope_3674 = format!("{}/PrincipalAvatar@3674", __ice_use_scope); ::ui_lang_runtime::rev_memo(446u64, (self.__ice_rev[74], __ice_palette.name), ::ui_lang_runtime::grow_stack(|| self.__ice_component_use_81(__ice_palette, __ice_component_05072696e636970616c417661746172_scope_3674))).into() };
#[cfg(test)]
drop(__ice_render_source);
__ice_rendered
}; let __container = {  ::iced::widget::container(__container_content).padding(::ui_lang_runtime::bounded_padding(1.0, 1.0, 1.0, 1.0)).style(move |__theme| { let mut __style = ::iced::widget::container::Style::default(); __style.background = ::std::option::Option::Some(::iced::Background::Color(__ice_palette.colors[93])); __style.border.radius = ::iced::border::Radius { top_left: ((16.5) as f32).max(0.0).min(f32::MAX), top_right: ((16.5) as f32).max(0.0).min(f32::MAX), bottom_right: ((16.5) as f32).max(0.0).min(f32::MAX), bottom_left: ((16.5) as f32).max(0.0).min(f32::MAX) }; __style }) }; ::ui_lang_runtime::accessible(__container, ::ui_lang_runtime::StableId::new(&__a11y_key), ::ui_lang_runtime::Role::GenericContainer).logical_id_maybe(::core::cfg!(test).then_some(__a11y_key)).into() };
#[cfg(test)]
drop(__ice_render_source);
__ice_rendered
}; let __button = ::iced::widget::button(__button_content).padding(0.0 as f32).on_press_maybe(if __disabled { None } else { Some(__activate.clone()) }).style(move |__theme, __status| { let mut __style = ::iced::widget::button::primary(__theme, __status); let __background: Option<::iced::Color> = match __status { ::iced::widget::button::Status::Hovered => Some(__ice_palette.colors[14]), ::iced::widget::button::Status::Pressed => Some(__ice_palette.colors[39]), ::iced::widget::button::Status::Disabled => Some(::iced::Color::from_rgba8(0, 0, 0, 0.000000)), _ => Some(::iced::Color::from_rgba8(0, 0, 0, 0.000000)) }; if let Some(__background) = __background { __style.background = Some(::iced::Background::Color(__background)); } __style.text_color = __ice_palette.colors[4]; __style.border.radius = 7.0.into(); __style.background = ::std::option::Option::Some(::iced::Background::Color(::iced::Color::from_rgba8(0, 0, 0, 0.000000))); __style.text_color = __ice_palette.colors[5]; __style.border.color = ::iced::Color::from_rgba8(0, 0, 0, 0.000000); __style.border.width = 1.0 as f32; __style.border.radius = ::iced::border::Radius { top_left: ((16.5) as f32).max(0.0).min(f32::MAX), top_right: ((16.5) as f32).max(0.0).min(f32::MAX), bottom_right: ((16.5) as f32).max(0.0).min(f32::MAX), bottom_left: ((16.5) as f32).max(0.0).min(f32::MAX) }; match __status { ::iced::widget::button::Status::Hovered => { __style.background = ::std::option::Option::Some(::iced::Background::Color(__ice_palette.colors[58])); __style.text_color = __ice_palette.colors[4]; } ::iced::widget::button::Status::Pressed => { __style.background = ::std::option::Option::Some(::iced::Background::Color(__ice_palette.colors[56])); __style.text_color = __ice_palette.colors[4]; } _ => {} } if matches!(__status, ::iced::widget::button::Status::Disabled) { if let Some(::iced::Background::Color(mut __color)) = __style.background { __color.a *= 0.5; __style.background = Some(::iced::Background::Color(__color)); } __style.text_color.a *= 0.5; } __style }); ::ui_lang_runtime::accessible(__button, __a11y_id, ::ui_lang_runtime::Role::Button).logical_id_maybe(::core::cfg!(test).then_some(&*__a11y_key)).focus_id(::iced::widget::Id::from(__a11y_key)).label("Account".to_owned()).disabled(__disabled).on_activate_maybe(if __disabled { None } else { Some(__activate) }).focus_ring(__ice_palette.colors[42], 7.0).into() };
#[cfg(test)]
drop(__ice_render_source);
__ice_rendered
}); let __child_count = __children.len(); let __children = __children.into_iter().map(|__child| ::ui_lang_runtime::bounded_fill_element(__child, __child_count, false)).collect::<::std::vec::Vec<_>>(); let __layout = ::iced::widget::column(__children).spacing(::ui_lang_runtime::bounded_spacing(0.0, __child_count)).width(::iced::Fill).height(::iced::Fill).align_x(::iced::alignment::Horizontal::Center); let __content = ::iced::widget::container(__layout); let __layout_content: __IceElement<'_, __DucktapeMessage> = __content.into(); let __a11y_key = format!("{}/@layout:687", __ice_use_scope); ::ui_lang_runtime::accessible(__layout_content, ::ui_lang_runtime::StableId::new(&__a11y_key), ::ui_lang_runtime::Role::GenericContainer).logical_id_maybe(::core::cfg!(test).then_some(__a11y_key)).into() };
#[cfg(test)]
drop(__ice_render_source);
__ice_rendered
}; let __container = {  ::iced::widget::container(__container_content).id(::iced::widget::Id::from(__ice_node_scope.clone())).padding(::ui_lang_runtime::bounded_padding(13.0, 0.0, 10.0, 0.0)).width(74.0 as f32).height(::iced::Fill).clip(true).style(move |__theme| { let mut __style = ::iced::widget::container::Style::default(); __style.background = ::std::option::Option::Some(::iced::Background::Color(__ice_palette.colors[53])); __style }) }; ::ui_lang_runtime::accessible(__container, ::ui_lang_runtime::StableId::new(&__a11y_key), ::ui_lang_runtime::Role::GenericContainer).logical_id_maybe(::core::cfg!(test).then_some(__a11y_key)).into() } };
#[cfg(test)]
drop(__ice_render_source);
__ice_rendered
}; __component_content }
pub(super) fn __ice_component_use_83(&self, __ice_palette: __IcePalette, __ice_use_scope: ::std::string::String) -> __IceElement<'_, __DucktapeMessage> { let __component_content: __IceElement<'_, __DucktapeMessage> = {
#[cfg(test)]
let __ice_render_source_location = ::ui_lang_runtime::testing::Location::new("components/shell.ice", 532, 1, "rendered view node");
#[cfg(test)]
let __ice_render_source = ::ui_lang_runtime::testing::push_render_source(__ice_render_source_location);
let __ice_rendered: __IceElement<'_, __DucktapeMessage> = { let __ice_node_scope = format!("{}/root", __ice_use_scope); { let __a11y_key = __ice_node_scope.as_str(); let __container_content: __IceElement<'_, __DucktapeMessage> = {
#[cfg(test)]
let __ice_render_source_location = ::ui_lang_runtime::testing::Location::new("components/shell.ice", 541, 1, "rendered view node");
#[cfg(test)]
let __ice_render_source = ::ui_lang_runtime::testing::push_render_source(__ice_render_source_location);
let __ice_rendered: __IceElement<'_, __DucktapeMessage> = { let mut __children: ::std::vec::Vec<__IceElement<'_, __DucktapeMessage>> = ::std::vec::Vec::new(); __children.push({
#[cfg(test)]
let __ice_render_source_location = ::ui_lang_runtime::testing::Location::new("components/shell.ice", 547, 1, "rendered view node");
#[cfg(test)]
let __ice_render_source = ::ui_lang_runtime::testing::push_render_source(__ice_render_source_location);
let __ice_rendered: __IceElement<'_, __DucktapeMessage> = { let __a11y_key = format!("{}/@container:547", __ice_use_scope); let __container_content: __IceElement<'_, __DucktapeMessage> = {
#[cfg(test)]
let __ice_render_source_location = ::ui_lang_runtime::testing::Location::new("components/shell.ice", 553, 1, "rendered view node");
#[cfg(test)]
let __ice_render_source = ::ui_lang_runtime::testing::push_render_source(__ice_render_source_location);
let __ice_rendered: __IceElement<'_, __DucktapeMessage> = ::iced::widget::space().width(1.0 as f32).height(1.0 as f32).into();
#[cfg(test)]
drop(__ice_render_source);
__ice_rendered
}; let __container = {  ::iced::widget::container(__container_content).width(7.0 as f32).height(7.0 as f32).style(move |__theme| { let mut __style = ::iced::widget::container::Style::default(); __style.background = ::std::option::Option::Some(::iced::Background::Color(__ice_palette.colors[83])); __style.border.radius = ::iced::border::Radius { top_left: ((3.5) as f32).max(0.0).min(f32::MAX), top_right: ((3.5) as f32).max(0.0).min(f32::MAX), bottom_right: ((3.5) as f32).max(0.0).min(f32::MAX), bottom_left: ((3.5) as f32).max(0.0).min(f32::MAX) }; __style }) }; ::ui_lang_runtime::accessible(__container, ::ui_lang_runtime::StableId::new(&__a11y_key), ::ui_lang_runtime::Role::GenericContainer).logical_id_maybe(::core::cfg!(test).then_some(__a11y_key)).into() };
#[cfg(test)]
drop(__ice_render_source);
__ice_rendered
}); __children.push({
#[cfg(test)]
let __ice_render_source_location = ::ui_lang_runtime::testing::Location::new("components/shell.ice", 554, 1, "rendered view node");
#[cfg(test)]
let __ice_render_source = ::ui_lang_runtime::testing::push_render_source(__ice_render_source_location);
let __ice_rendered: __IceElement<'_, __DucktapeMessage> = { let __a11y_key = format!("{}/@text:554", __ice_use_scope); let __text_value = ("Connection degraded").to_string(); let __text = ::iced::widget::text(__text_value.clone()).size(((13.0) as f32).max(f32::EPSILON).min(f32::MAX)).wrapping(::iced::widget::text::Wrapping::None).font(::iced::Font { family: ::iced::font::Family::Name("Geist"), weight: ::iced::font::Weight::Medium, stretch: ::iced::font::Stretch::Normal, style: ::iced::font::Style::Normal }).color(__ice_palette.colors[80]); let __text = ::ui_lang_runtime::selectable_text(__text); ::ui_lang_runtime::accessible(__text, ::ui_lang_runtime::StableId::new(&__a11y_key), ::ui_lang_runtime::Role::Label).logical_id_maybe(::core::cfg!(test).then_some(__a11y_key)).value(__text_value).into() };
#[cfg(test)]
drop(__ice_render_source);
__ice_rendered
}); __children.push({
#[cfg(test)]
let __ice_render_source_location = ::ui_lang_runtime::testing::Location::new("components/shell.ice", 560, 1, "rendered view node");
#[cfg(test)]
let __ice_render_source = ::ui_lang_runtime::testing::push_render_source(__ice_render_source_location);
let __ice_rendered: __IceElement<'_, __DucktapeMessage> = { let __a11y_key = format!("{}/@text:560", __ice_use_scope); let __text_value = (self.status).to_string(); let __text = ::iced::widget::text(__text_value.clone()).size(((13.0) as f32).max(f32::EPSILON).min(f32::MAX)).width(::iced::Fill).color(__ice_palette.colors[70]); let __text = ::ui_lang_runtime::selectable_text(__text); ::ui_lang_runtime::accessible(__text, ::ui_lang_runtime::StableId::new(&__a11y_key), ::ui_lang_runtime::Role::Label).logical_id_maybe(::core::cfg!(test).then_some(__a11y_key)).value(__text_value).into() };
#[cfg(test)]
drop(__ice_render_source);
__ice_rendered
}); let __child_count = __children.len(); let __children = __children.into_iter().map(|__child| ::ui_lang_runtime::bounded_fill_element(__child, __child_count, true)).collect::<::std::vec::Vec<_>>(); let __layout = ::iced::widget::row(__children).spacing(::ui_lang_runtime::bounded_spacing(8.0, __child_count)).width(::iced::Fill).height(::iced::Fill).align_y(::iced::alignment::Vertical::Center); let __content = ::iced::widget::container(__layout); let __layout_content: __IceElement<'_, __DucktapeMessage> = __content.into(); let __a11y_key = format!("{}/@layout:541", __ice_use_scope); ::ui_lang_runtime::accessible(__layout_content, ::ui_lang_runtime::StableId::new(&__a11y_key), ::ui_lang_runtime::Role::GenericContainer).logical_id_maybe(::core::cfg!(test).then_some(__a11y_key)).into() };
#[cfg(test)]
drop(__ice_render_source);
__ice_rendered
}; let __container = {  ::iced::widget::container(__container_content).id(::iced::widget::Id::from(__ice_node_scope.clone())).padding(::ui_lang_runtime::bounded_padding(0.0, 14.0, 0.0, 14.0)).width(::iced::Fill).height(30.0 as f32).style(move |__theme| { let mut __style = ::iced::widget::container::Style::default(); __style.background = ::std::option::Option::Some(::iced::Background::Color(__ice_palette.colors[81])); __style.border.color = __ice_palette.colors[82]; __style.border.width = 1.0 as f32; __style }) }; ::ui_lang_runtime::accessible(__container, ::ui_lang_runtime::StableId::new(&__a11y_key), ::ui_lang_runtime::Role::GenericContainer).logical_id_maybe(::core::cfg!(test).then_some(__a11y_key)).into() } };
#[cfg(test)]
drop(__ice_render_source);
__ice_rendered
}; __component_content }
pub(super) fn __ice_component_use_96(&self, __ice_palette: __IcePalette, __ice_use_scope: ::std::string::String) -> __IceElement<'_, __DucktapeMessage> { let __component_content: __IceElement<'_, __DucktapeMessage> = {
#[cfg(test)]
let __ice_render_source_location = ::ui_lang_runtime::testing::Location::new("components/shell.ice", 855, 1, "rendered view node");
#[cfg(test)]
let __ice_render_source = ::ui_lang_runtime::testing::push_render_source(__ice_render_source_location);
let __ice_rendered: __IceElement<'_, __DucktapeMessage> = { let __a11y_key = format!("{}/@container:855", __ice_use_scope); let __container_content: __IceElement<'_, __DucktapeMessage> = {
#[cfg(test)]
let __ice_render_source_location = ::ui_lang_runtime::testing::Location::new("components/shell.ice", 862, 1, "rendered view node");
#[cfg(test)]
let __ice_render_source = ::ui_lang_runtime::testing::push_render_source(__ice_render_source_location);
let __ice_rendered: __IceElement<'_, __DucktapeMessage> = { let mut __children: ::std::vec::Vec<__IceElement<'_, __DucktapeMessage>> = ::std::vec::Vec::new(); __children.push({
#[cfg(test)]
let __ice_render_source_location = ::ui_lang_runtime::testing::Location::new("components/shell.ice", 863, 1, "rendered view node");
#[cfg(test)]
let __ice_render_source = ::ui_lang_runtime::testing::push_render_source(__ice_render_source_location);
let __ice_rendered: __IceElement<'_, __DucktapeMessage> = { let mut __children: ::std::vec::Vec<__IceElement<'_, __DucktapeMessage>> = ::std::vec::Vec::new(); __children.push({
#[cfg(test)]
let __ice_render_source_location = ::ui_lang_runtime::testing::Location::new("components/shell.ice", 864, 1, "rendered view node");
#[cfg(test)]
let __ice_render_source = ::ui_lang_runtime::testing::push_render_source(__ice_render_source_location);
let __ice_rendered: __IceElement<'_, __DucktapeMessage> = { let __ice_node_scope = format!("{}/titlebar", __ice_use_scope); ::ui_lang_runtime::grow_stack(|| self.__ice_component_use_72(__ice_palette, __ice_node_scope.clone(), (move || __DucktapeMessage::SwitchNetwork).clone(), (move || __DucktapeMessage::ToggleBell).clone())) };
#[cfg(test)]
drop(__ice_render_source);
__ice_rendered
}); __children.push({
#[cfg(test)]
let __ice_render_source_location = ::ui_lang_runtime::testing::Location::new("components/shell.ice", 883, 1, "rendered view node");
#[cfg(test)]
let __ice_render_source = ::ui_lang_runtime::testing::push_render_source(__ice_render_source_location);
let __ice_rendered: __IceElement<'_, __DucktapeMessage> = { let mut __children: ::std::vec::Vec<__IceElement<'_, __DucktapeMessage>> = ::std::vec::Vec::new(); __children.push({
#[cfg(test)]
let __ice_render_source_location = ::ui_lang_runtime::testing::Location::new("components/shell.ice", 884, 1, "rendered view node");
#[cfg(test)]
let __ice_render_source = ::ui_lang_runtime::testing::push_render_source(__ice_render_source_location);
let __ice_rendered: __IceElement<'_, __DucktapeMessage> = { let __ice_node_scope = format!("{}/rail", __ice_use_scope); ::ui_lang_runtime::grow_stack(|| self.__ice_component_use_82(__ice_palette, __ice_node_scope.clone(), (move |__event_0| __DucktapeMessage::SelectShellTab(__event_0)).clone())) };
#[cfg(test)]
drop(__ice_render_source);
__ice_rendered
}); __children.push({
#[cfg(test)]
let __ice_render_source_location = ::ui_lang_runtime::testing::Location::new("components/shell.ice", 892, 1, "rendered view node");
#[cfg(test)]
let __ice_render_source = ::ui_lang_runtime::testing::push_render_source(__ice_render_source_location);
let __ice_rendered: __IceElement<'_, __DucktapeMessage> = { let __a11y_key = format!("{}/@container:892", __ice_use_scope); let __container_content: __IceElement<'_, __DucktapeMessage> = {
#[cfg(test)]
let __ice_render_source_location = ::ui_lang_runtime::testing::Location::new("components/shell.ice", 897, 1, "rendered view node");
#[cfg(test)]
let __ice_render_source = ::ui_lang_runtime::testing::push_render_source(__ice_render_source_location);
let __ice_rendered: __IceElement<'_, __DucktapeMessage> = ::iced::widget::space().width(1.0 as f32).height(1.0 as f32).into();
#[cfg(test)]
drop(__ice_render_source);
__ice_rendered
}; let __container = {  ::iced::widget::container(__container_content).width(1.0 as f32).height(::iced::Fill).style(move |__theme| { let mut __style = ::iced::widget::container::Style::default(); __style.background = ::std::option::Option::Some(::iced::Background::Color(__ice_palette.colors[60])); __style }) }; ::ui_lang_runtime::accessible(__container, ::ui_lang_runtime::StableId::new(&__a11y_key), ::ui_lang_runtime::Role::GenericContainer).logical_id_maybe(::core::cfg!(test).then_some(__a11y_key)).into() };
#[cfg(test)]
drop(__ice_render_source);
__ice_rendered
}); __children.push({
#[cfg(test)]
let __ice_render_source_location = ::ui_lang_runtime::testing::Location::new("components/shell.ice", 898, 1, "rendered view node");
#[cfg(test)]
let __ice_render_source = ::ui_lang_runtime::testing::push_render_source(__ice_render_source_location);
let __ice_rendered: __IceElement<'_, __DucktapeMessage> = { let __ice_node_scope = format!("{}/content", __ice_use_scope); { let __a11y_key = __ice_node_scope.as_str(); let __container_content: __IceElement<'_, __DucktapeMessage> = {
#[cfg(test)]
let __ice_render_source_location = ::ui_lang_runtime::testing::Location::new("components/shell.ice", 904, 1, "rendered view node");
#[cfg(test)]
let __ice_render_source = ::ui_lang_runtime::testing::push_render_source(__ice_render_source_location);
let __ice_rendered: __IceElement<'_, __DucktapeMessage> = { let mut __children: ::std::vec::Vec<__IceElement<'_, __DucktapeMessage>> = ::std::vec::Vec::new(); if crate::backend::connection_degraded(::std::convert::AsRef::as_ref(&(self.status))) { __children.push({
#[cfg(test)]
let __ice_render_source_location = ::ui_lang_runtime::testing::Location::new("components/shell.ice", 906, 1, "rendered view node");
#[cfg(test)]
let __ice_render_source = ::ui_lang_runtime::testing::push_render_source(__ice_render_source_location);
let __ice_rendered: __IceElement<'_, __DucktapeMessage> = { let __ice_component_0436f6e6e656374696f6e42616e6e6572_scope_3826 = format!("{}/ConnectionBanner@3826", __ice_use_scope); ::ui_lang_runtime::rev_memo(476u64, (self.__ice_rev[7], __ice_palette.name), ::ui_lang_runtime::grow_stack(|| self.__ice_component_use_83(__ice_palette, __ice_component_0436f6e6e656374696f6e42616e6e6572_scope_3826))).into() };
#[cfg(test)]
drop(__ice_render_source);
__ice_rendered
}); } __children.push({
#[cfg(test)]
let __ice_render_source_location = ::ui_lang_runtime::testing::Location::new("components/shell.ice", 907, 1, "rendered view node");
#[cfg(test)]
let __ice_render_source = ::ui_lang_runtime::testing::push_render_source(__ice_render_source_location);
let __ice_rendered: __IceElement<'_, __DucktapeMessage> = (|| { let __slot_content: __IceElement<'_, __DucktapeMessage> = {
#[cfg(test)]
let __ice_render_source_location = ::ui_lang_runtime::testing::Location::new("view.ice", 135, 1, "rendered view node");
#[cfg(test)]
let __ice_render_source = ::ui_lang_runtime::testing::push_render_source(__ice_render_source_location);
let __ice_rendered: __IceElement<'_, __DucktapeMessage> = { let mut __children: ::std::vec::Vec<__IceElement<'_, __DucktapeMessage>> = ::std::vec::Vec::new(); __children.push({
#[cfg(test)]
let __ice_render_source_location = ::ui_lang_runtime::testing::Location::new("view.ice", 136, 1, "rendered view node");
#[cfg(test)]
let __ice_render_source = ::ui_lang_runtime::testing::push_render_source(__ice_render_source_location);
let __ice_rendered: __IceElement<'_, __DucktapeMessage> = { let __ice_node_scope = format!("{}/account-banner", __ice_node_scope); { let __ice_component_04163636f756e7442616e6e6572_scope_10346 = __ice_node_scope.clone(); ::ui_lang_runtime::rev_memo(1110u64, (self.__ice_rev[6], self.__ice_rev[72], self.__ice_rev[79], self.__ice_rev[8], __ice_palette.name), ::ui_lang_runtime::grow_stack(|| self.__ice_component_use_84(__ice_palette, __ice_component_04163636f756e7442616e6e6572_scope_10346))).into() } };
#[cfg(test)]
drop(__ice_render_source);
__ice_rendered
}); if (*self.__ice_derived_has_error()) { __children.push({
#[cfg(test)]
let __ice_render_source_location = ::ui_lang_runtime::testing::Location::new("view.ice", 146, 1, "rendered view node");
#[cfg(test)]
let __ice_render_source = ::ui_lang_runtime::testing::push_render_source(__ice_render_source_location);
let __ice_rendered: __IceElement<'_, __DucktapeMessage> = { let __a11y_key = format!("{}/@container:146", __ice_node_scope); let __container_content: __IceElement<'_, __DucktapeMessage> = {
#[cfg(test)]
let __ice_render_source_location = ::ui_lang_runtime::testing::Location::new("view.ice", 152, 1, "rendered view node");
#[cfg(test)]
let __ice_render_source = ::ui_lang_runtime::testing::push_render_source(__ice_render_source_location);
let __ice_rendered: __IceElement<'_, __DucktapeMessage> = { let __a11y_key = format!("{}/@container:152", __ice_node_scope); let __container_content: __IceElement<'_, __DucktapeMessage> = {
#[cfg(test)]
let __ice_render_source_location = ::ui_lang_runtime::testing::Location::new("view.ice", 160, 1, "rendered view node");
#[cfg(test)]
let __ice_render_source = ::ui_lang_runtime::testing::push_render_source(__ice_render_source_location);
let __ice_rendered: __IceElement<'_, __DucktapeMessage> = { let mut __children: ::std::vec::Vec<__IceElement<'_, __DucktapeMessage>> = ::std::vec::Vec::new(); __children.push({
#[cfg(test)]
let __ice_render_source_location = ::ui_lang_runtime::testing::Location::new("view.ice", 165, 1, "rendered view node");
#[cfg(test)]
let __ice_render_source = ::ui_lang_runtime::testing::push_render_source(__ice_render_source_location);
let __ice_rendered: __IceElement<'_, __DucktapeMessage> = { let __a11y_key = format!("{}/@container:165", __ice_node_scope); let __container_content: __IceElement<'_, __DucktapeMessage> = {
#[cfg(test)]
let __ice_render_source_location = ::ui_lang_runtime::testing::Location::new("view.ice", 173, 1, "rendered view node");
#[cfg(test)]
let __ice_render_source = ::ui_lang_runtime::testing::push_render_source(__ice_render_source_location);
let __ice_rendered: __IceElement<'_, __DucktapeMessage> = { let __a11y_key = format!("{}/@text:173", __ice_node_scope); let __text_value = ("!").to_string(); let __text = ::iced::widget::text(__text_value.clone()).size(((14.0) as f32).max(f32::EPSILON).min(f32::MAX)).font(::iced::Font { family: ::iced::font::Family::Name("Geist"), weight: ::iced::font::Weight::Medium, stretch: ::iced::font::Stretch::Normal, style: ::iced::font::Style::Normal }).color(__ice_palette.colors[21]); let __text = ::ui_lang_runtime::selectable_text(__text); ::ui_lang_runtime::accessible(__text, ::ui_lang_runtime::StableId::new(&__a11y_key), ::ui_lang_runtime::Role::Label).logical_id_maybe(::core::cfg!(test).then_some(__a11y_key)).value(__text_value).into() };
#[cfg(test)]
drop(__ice_render_source);
__ice_rendered
}; let __container = {  ::iced::widget::container(__container_content).width(20.0 as f32).height(20.0 as f32).align_x(::iced::alignment::Horizontal::Center).align_y(::iced::alignment::Vertical::Center).style(move |__theme| { let mut __style = ::iced::widget::container::Style::default(); __style.background = ::std::option::Option::Some(::iced::Background::Color(__ice_palette.colors[24])); __style.border.radius = ::iced::border::Radius { top_left: ((10.0) as f32).max(0.0).min(f32::MAX), top_right: ((10.0) as f32).max(0.0).min(f32::MAX), bottom_right: ((10.0) as f32).max(0.0).min(f32::MAX), bottom_left: ((10.0) as f32).max(0.0).min(f32::MAX) }; __style }) }; ::ui_lang_runtime::accessible(__container, ::ui_lang_runtime::StableId::new(&__a11y_key), ::ui_lang_runtime::Role::GenericContainer).logical_id_maybe(::core::cfg!(test).then_some(__a11y_key)).into() };
#[cfg(test)]
drop(__ice_render_source);
__ice_rendered
}); __children.push({
#[cfg(test)]
let __ice_render_source_location = ::ui_lang_runtime::testing::Location::new("view.ice", 178, 1, "rendered view node");
#[cfg(test)]
let __ice_render_source = ::ui_lang_runtime::testing::push_render_source(__ice_render_source_location);
let __ice_rendered: __IceElement<'_, __DucktapeMessage> = { let __a11y_key = format!("{}/@text:178", __ice_node_scope); let __text_value = (self.error).to_string(); let __text = ::iced::widget::text(__text_value.clone()).size(((13.5) as f32).max(f32::EPSILON).min(f32::MAX)).width(::iced::Fill).color(__ice_palette.colors[4]); let __text = ::ui_lang_runtime::selectable_text(__text); ::ui_lang_runtime::accessible(__text, ::ui_lang_runtime::StableId::new(&__a11y_key), ::ui_lang_runtime::Role::Label).logical_id_maybe(::core::cfg!(test).then_some(__a11y_key)).value(__text_value).into() };
#[cfg(test)]
drop(__ice_render_source);
__ice_rendered
}); __children.push({
#[cfg(test)]
let __ice_render_source_location = ::ui_lang_runtime::testing::Location::new("view.ice", 183, 1, "rendered view node");
#[cfg(test)]
let __ice_render_source = ::ui_lang_runtime::testing::push_render_source(__ice_render_source_location);
let __ice_rendered: __IceElement<'_, __DucktapeMessage> = { let __a11y_key = format!("{}/@button:183", __ice_node_scope); let __a11y_id = ::ui_lang_runtime::StableId::new(&__a11y_key); let __disabled = false; let __activate = __DucktapeMessage::DismissError; let __button_content: __IceElement<'_, __DucktapeMessage> = ::iced::widget::text("Dismiss").size(12.5).font(::iced::Font { weight: ::iced::font::Weight::Semibold, ..Self::default_font() }).into(); let __button = ::iced::widget::button(__button_content).padding(::iced::Padding { top: 7.0, right: 12.0, bottom: 7.0, left: 12.0 }).padding(5.0 as f32).on_press_maybe(if __disabled { None } else { Some(__activate.clone()) }).style(move |__theme, __status| { let mut __style = ::iced::widget::button::primary(__theme, __status); let __background: Option<::iced::Color> = match __status { ::iced::widget::button::Status::Hovered => Some(__ice_palette.colors[14]), ::iced::widget::button::Status::Pressed => Some(__ice_palette.colors[39]), ::iced::widget::button::Status::Disabled => Some(::iced::Color::from_rgba8(0, 0, 0, 0.000000)), _ => Some(::iced::Color::from_rgba8(0, 0, 0, 0.000000)) }; if let Some(__background) = __background { __style.background = Some(::iced::Background::Color(__background)); } __style.text_color = __ice_palette.colors[4]; __style.border.radius = 8.0.into(); __style.background = ::std::option::Option::Some(::iced::Background::Color(::iced::Color::from_rgba8(0, 0, 0, 0.000000))); __style.text_color = __ice_palette.colors[5]; __style.border.radius = ::iced::border::Radius { top_left: ((7.0) as f32).max(0.0).min(f32::MAX), top_right: ((7.0) as f32).max(0.0).min(f32::MAX), bottom_right: ((7.0) as f32).max(0.0).min(f32::MAX), bottom_left: ((7.0) as f32).max(0.0).min(f32::MAX) }; match __status { ::iced::widget::button::Status::Hovered => { __style.background = ::std::option::Option::Some(::iced::Background::Color({ let mut __color = __ice_palette.colors[4]; __color.a = 0.090000; __color })); __style.text_color = __ice_palette.colors[4]; } ::iced::widget::button::Status::Pressed => { __style.background = ::std::option::Option::Some(::iced::Background::Color({ let mut __color = __ice_palette.colors[4]; __color.a = 0.140000; __color })); } _ => {} } if matches!(__status, ::iced::widget::button::Status::Disabled) { if let Some(::iced::Background::Color(mut __color)) = __style.background { __color.a *= 0.5; __style.background = Some(::iced::Background::Color(__color)); } __style.text_color.a *= 0.5; } __style }); ::ui_lang_runtime::accessible(__button, __a11y_id, ::ui_lang_runtime::Role::Button).logical_id_maybe(::core::cfg!(test).then_some(&*__a11y_key)).focus_id(::iced::widget::Id::from(__a11y_key)).label("Dismiss").disabled(__disabled).on_activate_maybe(if __disabled { None } else { Some(__activate) }).focus_ring(__ice_palette.colors[42], 8.0).into() };
#[cfg(test)]
drop(__ice_render_source);
__ice_rendered
}); let __child_count = __children.len(); let __children = __children.into_iter().map(|__child| ::ui_lang_runtime::bounded_fill_element(__child, __child_count, true)).collect::<::std::vec::Vec<_>>(); let __layout = ::iced::widget::row(__children).spacing(::ui_lang_runtime::bounded_spacing(8.0, __child_count)).width(::iced::Fill).align_y(::iced::alignment::Vertical::Center); let __content = ::iced::widget::container(__layout); let __layout_content: __IceElement<'_, __DucktapeMessage> = __content.into(); let __a11y_key = format!("{}/@layout:160", __ice_node_scope); ::ui_lang_runtime::accessible(__layout_content, ::ui_lang_runtime::StableId::new(&__a11y_key), ::ui_lang_runtime::Role::GenericContainer).logical_id_maybe(::core::cfg!(test).then_some(__a11y_key)).into() };
#[cfg(test)]
drop(__ice_render_source);
__ice_rendered
}; let __container = {  ::iced::widget::container(__container_content).padding(::ui_lang_runtime::bounded_padding(8.0, 8.0, 8.0, 8.0)).width(::iced::Fill).style(move |__theme| { let mut __style = ::iced::widget::container::Style::default(); __style.background = ::std::option::Option::Some(::iced::Background::Color(__ice_palette.colors[22])); __style.border.color = __ice_palette.colors[23]; __style.border.width = 1.0 as f32; __style.border.radius = ::iced::border::Radius { top_left: ((12.0) as f32).max(0.0).min(f32::MAX), top_right: ((12.0) as f32).max(0.0).min(f32::MAX), bottom_right: ((12.0) as f32).max(0.0).min(f32::MAX), bottom_left: ((12.0) as f32).max(0.0).min(f32::MAX) }; __style }) }; ::ui_lang_runtime::accessible(__container, ::ui_lang_runtime::StableId::new(&__a11y_key), ::ui_lang_runtime::Role::GenericContainer).logical_id_maybe(::core::cfg!(test).then_some(__a11y_key)).into() };
#[cfg(test)]
drop(__ice_render_source);
__ice_rendered
}; let __container = {  ::iced::widget::container(__container_content).padding(::ui_lang_runtime::bounded_padding(0.0, 12.0, 8.0, 12.0)).width(::iced::Fill) }; ::ui_lang_runtime::accessible(__container, ::ui_lang_runtime::StableId::new(&__a11y_key), ::ui_lang_runtime::Role::GenericContainer).logical_id_maybe(::core::cfg!(test).then_some(__a11y_key)).into() };
#[cfg(test)]
drop(__ice_render_source);
__ice_rendered
}); } let __child_count = __children.len(); let __children = __children.into_iter().map(|__child| ::ui_lang_runtime::bounded_fill_element(__child, __child_count, false)).collect::<::std::vec::Vec<_>>(); let __layout = ::iced::widget::column(__children).width(::iced::Fill); let __content = ::iced::widget::container(__layout); let __layout_content: __IceElement<'_, __DucktapeMessage> = __content.into(); let __a11y_key = format!("{}/@layout:135", __ice_node_scope); ::ui_lang_runtime::accessible(__layout_content, ::ui_lang_runtime::StableId::new(&__a11y_key), ::ui_lang_runtime::Role::GenericContainer).logical_id_maybe(::core::cfg!(test).then_some(__a11y_key)).into() };
#[cfg(test)]
drop(__ice_render_source);
__ice_rendered
}; __slot_content })();
#[cfg(test)]
drop(__ice_render_source);
__ice_rendered
}); match &(self.shell_tab) { ShellTab::Chat => { __children.push({
#[cfg(test)]
let __ice_render_source_location = ::ui_lang_runtime::testing::Location::new("components/shell.ice", 910, 1, "rendered view node");
#[cfg(test)]
let __ice_render_source = ::ui_lang_runtime::testing::push_render_source(__ice_render_source_location);
let __ice_rendered: __IceElement<'_, __DucktapeMessage> = (|| { let __slot_content: __IceElement<'_, __DucktapeMessage> = {
#[cfg(test)]
let __ice_render_source_location = ::ui_lang_runtime::testing::Location::new("view.ice", 197, 1, "rendered view node");
#[cfg(test)]
let __ice_render_source = ::ui_lang_runtime::testing::push_render_source(__ice_render_source_location);
let __ice_rendered: __IceElement<'_, __DucktapeMessage> = { let __ice_node_scope = format!("{}/chat", __ice_node_scope); { let __identified: __IceElement<'_, __DucktapeMessage> = crate::module_view::chat_view((*self.__ice_derived_dark()), self.connected, ::std::convert::AsRef::as_ref(&(self.connected_rpc)), ::std::convert::AsRef::as_ref(&(self.network_name)), ::std::convert::AsRef::as_ref(&(self.network_chain_id)), ::std::convert::AsRef::as_ref(&(self.status)), self.block_height, ::std::convert::AsRef::as_ref(&(self.account_number)), ::std::convert::AsRef::as_ref(&(self.settings_user_key)), self.dm_peers_generation, ::std::convert::AsRef::as_ref(&(self.rooms)), ::std::convert::AsRef::as_ref(&(self.dm_rows)), self.channel_create_open, ::std::convert::AsRef::as_ref(&(self.active_channel)), ::std::convert::AsRef::as_ref(&(self.active_dm_peer)), ::std::borrow::Borrow::borrow(&(self.active_dm)), self.chat_land_seq, self.unread_boundary, self.mutation_phase.clone(), self.loading, self.huddle_joined, ::std::convert::AsRef::as_ref(&(self.huddle_channel)), ::std::convert::AsRef::as_ref(&(self.huddle_channel_name)), self.huddle_joined_at, self.huddle_now, self.call_muted, self.shift_held, self.chat_copy_chord_serial, self.chat_sent_serial, ::std::convert::AsRef::as_ref(&(self.chat_pending_sends)), ::std::convert::AsRef::as_ref(&(self.live_agents))).map(move |__value| __DucktapeMessage::ChatViewEvent(__value)).into(); #[cfg(test)] ::ui_lang_runtime::testing::register_render_source(&__ice_node_scope); ::iced::widget::container(__identified).id(::iced::widget::Id::from(__ice_node_scope.clone())).into() } };
#[cfg(test)]
let __ice_rendered = ::ui_lang_runtime::testing::sourced(__ice_rendered, __ice_render_source_location);
#[cfg(test)]
drop(__ice_render_source);
__ice_rendered
}; __slot_content })();
#[cfg(test)]
drop(__ice_render_source);
__ice_rendered
}); }, ShellTab::Pages => { __children.push({
#[cfg(test)]
let __ice_render_source_location = ::ui_lang_runtime::testing::Location::new("components/shell.ice", 912, 1, "rendered view node");
#[cfg(test)]
let __ice_render_source = ::ui_lang_runtime::testing::push_render_source(__ice_render_source_location);
let __ice_rendered: __IceElement<'_, __DucktapeMessage> = (|| { let __slot_content: __IceElement<'_, __DucktapeMessage> = {
#[cfg(test)]
let __ice_render_source_location = ::ui_lang_runtime::testing::Location::new("view.ice", 205, 1, "rendered view node");
#[cfg(test)]
let __ice_render_source = ::ui_lang_runtime::testing::push_render_source(__ice_render_source_location);
let __ice_rendered: __IceElement<'_, __DucktapeMessage> = { let __ice_node_scope = format!("{}/pages", __ice_node_scope); { let __identified: __IceElement<'_, __DucktapeMessage> = crate::module_view::pages_view((*self.__ice_derived_dark()), self.connected, ::std::convert::AsRef::as_ref(&(self.network_chain_id)), ::std::convert::AsRef::as_ref(&(self.page_route)), self.page_route_serial).map(move |__value| __DucktapeMessage::PagesViewEvent(__value)).into(); #[cfg(test)] ::ui_lang_runtime::testing::register_render_source(&__ice_node_scope); ::iced::widget::container(__identified).id(::iced::widget::Id::from(__ice_node_scope.clone())).into() } };
#[cfg(test)]
let __ice_rendered = ::ui_lang_runtime::testing::sourced(__ice_rendered, __ice_render_source_location);
#[cfg(test)]
drop(__ice_render_source);
__ice_rendered
}; __slot_content })();
#[cfg(test)]
drop(__ice_render_source);
__ice_rendered
}); }, ShellTab::Forge => { __children.push({
#[cfg(test)]
let __ice_render_source_location = ::ui_lang_runtime::testing::Location::new("components/shell.ice", 914, 1, "rendered view node");
#[cfg(test)]
let __ice_render_source = ::ui_lang_runtime::testing::push_render_source(__ice_render_source_location);
let __ice_rendered: __IceElement<'_, __DucktapeMessage> = (|| { let __slot_content: __IceElement<'_, __DucktapeMessage> = {
#[cfg(test)]
let __ice_render_source_location = ::ui_lang_runtime::testing::Location::new("view.ice", 238, 1, "rendered view node");
#[cfg(test)]
let __ice_render_source = ::ui_lang_runtime::testing::push_render_source(__ice_render_source_location);
let __ice_rendered: __IceElement<'_, __DucktapeMessage> = { let __ice_node_scope = format!("{}/forge", __ice_node_scope); { let __identified: __IceElement<'_, __DucktapeMessage> = crate::module_view::forge_view((*self.__ice_derived_dark()), self.connected, ::std::convert::AsRef::as_ref(&(self.network_name)), ::std::convert::AsRef::as_ref(&(self.account_bio)), ::std::convert::AsRef::as_ref(&(crate::backend::member_tier(::std::convert::AsRef::as_ref(&(self.members_rows))))), ::std::convert::AsRef::as_ref(&(self.network_chain_id)), ::std::convert::AsRef::as_ref(&(self.connected_rpc)), ::std::convert::AsRef::as_ref(&(self.forge_link)), self.forge_link_tick).map(move |__value| __DucktapeMessage::ForgeViewEvent(__value)).into(); #[cfg(test)] ::ui_lang_runtime::testing::register_render_source(&__ice_node_scope); ::iced::widget::container(__identified).id(::iced::widget::Id::from(__ice_node_scope.clone())).into() } };
#[cfg(test)]
let __ice_rendered = ::ui_lang_runtime::testing::sourced(__ice_rendered, __ice_render_source_location);
#[cfg(test)]
drop(__ice_render_source);
__ice_rendered
}; __slot_content })();
#[cfg(test)]
drop(__ice_render_source);
__ice_rendered
}); }, ShellTab::Agents => { __children.push({
#[cfg(test)]
let __ice_render_source_location = ::ui_lang_runtime::testing::Location::new("components/shell.ice", 916, 1, "rendered view node");
#[cfg(test)]
let __ice_render_source = ::ui_lang_runtime::testing::push_render_source(__ice_render_source_location);
let __ice_rendered: __IceElement<'_, __DucktapeMessage> = (|| { let __slot_content: __IceElement<'_, __DucktapeMessage> = {
#[cfg(test)]
let __ice_render_source_location = ::ui_lang_runtime::testing::Location::new("view.ice", 229, 1, "rendered view node");
#[cfg(test)]
let __ice_render_source = ::ui_lang_runtime::testing::push_render_source(__ice_render_source_location);
let __ice_rendered: __IceElement<'_, __DucktapeMessage> = { let __ice_node_scope = format!("{}/agents", __ice_node_scope); { let __identified: __IceElement<'_, __DucktapeMessage> = crate::module_view::agents_view((*self.__ice_derived_dark()), self.connected, ::std::convert::AsRef::as_ref(&(self.account_number)), ::std::convert::AsRef::as_ref(&(self.agents_open_run)), self.agents_opened).map(move |__value| __DucktapeMessage::AgentsViewEvent(__value)).into(); #[cfg(test)] ::ui_lang_runtime::testing::register_render_source(&__ice_node_scope); ::iced::widget::container(__identified).id(::iced::widget::Id::from(__ice_node_scope.clone())).into() } };
#[cfg(test)]
let __ice_rendered = ::ui_lang_runtime::testing::sourced(__ice_rendered, __ice_render_source_location);
#[cfg(test)]
drop(__ice_render_source);
__ice_rendered
}; __slot_content })();
#[cfg(test)]
drop(__ice_render_source);
__ice_rendered
}); }, ShellTab::Files => { __children.push({
#[cfg(test)]
let __ice_render_source_location = ::ui_lang_runtime::testing::Location::new("components/shell.ice", 918, 1, "rendered view node");
#[cfg(test)]
let __ice_render_source = ::ui_lang_runtime::testing::push_render_source(__ice_render_source_location);
let __ice_rendered: __IceElement<'_, __DucktapeMessage> = (|| { let __slot_content: __IceElement<'_, __DucktapeMessage> = {
#[cfg(test)]
let __ice_render_source_location = ::ui_lang_runtime::testing::Location::new("view.ice", 214, 1, "rendered view node");
#[cfg(test)]
let __ice_render_source = ::ui_lang_runtime::testing::push_render_source(__ice_render_source_location);
let __ice_rendered: __IceElement<'_, __DucktapeMessage> = { let __ice_node_scope = format!("{}/files", __ice_node_scope); { let __identified: __IceElement<'_, __DucktapeMessage> = crate::module_view::files_view((*self.__ice_derived_dark()), self.connected, ::std::convert::AsRef::as_ref(&(self.network_chain_id)), ::std::convert::AsRef::as_ref(&(self.fs_route)), self.fs_route_serial).map(move |__value| __DucktapeMessage::FilesViewEvent(__value)).into(); #[cfg(test)] ::ui_lang_runtime::testing::register_render_source(&__ice_node_scope); ::iced::widget::container(__identified).id(::iced::widget::Id::from(__ice_node_scope.clone())).into() } };
#[cfg(test)]
let __ice_rendered = ::ui_lang_runtime::testing::sourced(__ice_rendered, __ice_render_source_location);
#[cfg(test)]
drop(__ice_render_source);
__ice_rendered
}; __slot_content })();
#[cfg(test)]
drop(__ice_render_source);
__ice_rendered
}); }, ShellTab::Explorer => { __children.push({
#[cfg(test)]
let __ice_render_source_location = ::ui_lang_runtime::testing::Location::new("components/shell.ice", 920, 1, "rendered view node");
#[cfg(test)]
let __ice_render_source = ::ui_lang_runtime::testing::push_render_source(__ice_render_source_location);
let __ice_rendered: __IceElement<'_, __DucktapeMessage> = (|| { let __slot_content: __IceElement<'_, __DucktapeMessage> = {
#[cfg(test)]
let __ice_render_source_location = ::ui_lang_runtime::testing::Location::new("view.ice", 268, 1, "rendered view node");
#[cfg(test)]
let __ice_render_source = ::ui_lang_runtime::testing::push_render_source(__ice_render_source_location);
let __ice_rendered: __IceElement<'_, __DucktapeMessage> = { let __ice_node_scope = format!("{}/explorer", __ice_node_scope); { let __identified: __IceElement<'_, __DucktapeMessage> = crate::module_view::explorer_view((*self.__ice_derived_dark()), self.connected, self.block_height, ::std::convert::AsRef::as_ref(&(crate::backend::sync_label(::std::convert::AsRef::as_ref(&(self.node_phase)), self.node_sync_applied, self.node_sync_target)))).map(move |__value| __DucktapeMessage::ExplorerViewEvent(__value)).into(); #[cfg(test)] ::ui_lang_runtime::testing::register_render_source(&__ice_node_scope); ::iced::widget::container(__identified).id(::iced::widget::Id::from(__ice_node_scope.clone())).into() } };
#[cfg(test)]
let __ice_rendered = ::ui_lang_runtime::testing::sourced(__ice_rendered, __ice_render_source_location);
#[cfg(test)]
drop(__ice_render_source);
__ice_rendered
}; __slot_content })();
#[cfg(test)]
drop(__ice_render_source);
__ice_rendered
}); }, ShellTab::Node => { __children.push({
#[cfg(test)]
let __ice_render_source_location = ::ui_lang_runtime::testing::Location::new("components/shell.ice", 922, 1, "rendered view node");
#[cfg(test)]
let __ice_render_source = ::ui_lang_runtime::testing::push_render_source(__ice_render_source_location);
let __ice_rendered: __IceElement<'_, __DucktapeMessage> = (|| { let __slot_content: __IceElement<'_, __DucktapeMessage> = {
#[cfg(test)]
let __ice_render_source_location = ::ui_lang_runtime::testing::Location::new("view.ice", 255, 1, "rendered view node");
#[cfg(test)]
let __ice_render_source = ::ui_lang_runtime::testing::push_render_source(__ice_render_source_location);
let __ice_rendered: __IceElement<'_, __DucktapeMessage> = { let __ice_node_scope = format!("{}/node", __ice_node_scope); { let __identified: __IceElement<'_, __DucktapeMessage> = crate::module_view::node_view((*self.__ice_derived_dark()), self.connected, crate::backend::members_is_admin(::std::convert::AsRef::as_ref(&(self.members_rows))), ::std::convert::AsRef::as_ref(&(crate::backend::member_tier(::std::convert::AsRef::as_ref(&(self.members_rows))))), ::std::convert::AsRef::as_ref(&(self.status)), ::std::convert::AsRef::as_ref(&(self.node_data_dir)), self.wall_now).map(move |__value| __DucktapeMessage::NodeViewEvent(__value)).into(); #[cfg(test)] ::ui_lang_runtime::testing::register_render_source(&__ice_node_scope); ::iced::widget::container(__identified).id(::iced::widget::Id::from(__ice_node_scope.clone())).into() } };
#[cfg(test)]
let __ice_rendered = ::ui_lang_runtime::testing::sourced(__ice_rendered, __ice_render_source_location);
#[cfg(test)]
drop(__ice_render_source);
__ice_rendered
}; __slot_content })();
#[cfg(test)]
drop(__ice_render_source);
__ice_rendered
}); }, ShellTab::Members => { __children.push({
#[cfg(test)]
let __ice_render_source_location = ::ui_lang_runtime::testing::Location::new("components/shell.ice", 924, 1, "rendered view node");
#[cfg(test)]
let __ice_render_source = ::ui_lang_runtime::testing::push_render_source(__ice_render_source_location);
let __ice_rendered: __IceElement<'_, __DucktapeMessage> = (|| { let __slot_content: __IceElement<'_, __DucktapeMessage> = {
#[cfg(test)]
let __ice_render_source_location = ::ui_lang_runtime::testing::Location::new("view.ice", 222, 1, "rendered view node");
#[cfg(test)]
let __ice_render_source = ::ui_lang_runtime::testing::push_render_source(__ice_render_source_location);
let __ice_rendered: __IceElement<'_, __DucktapeMessage> = { let __ice_node_scope = format!("{}/members", __ice_node_scope); { let __identified: __IceElement<'_, __DucktapeMessage> = crate::module_view::members_view((*self.__ice_derived_dark()), self.connected, crate::backend::members_is_admin(::std::convert::AsRef::as_ref(&(self.members_rows)))).map(move |__value| __DucktapeMessage::MembersViewEvent(__value)).into(); #[cfg(test)] ::ui_lang_runtime::testing::register_render_source(&__ice_node_scope); ::iced::widget::container(__identified).id(::iced::widget::Id::from(__ice_node_scope.clone())).into() } };
#[cfg(test)]
let __ice_rendered = ::ui_lang_runtime::testing::sourced(__ice_rendered, __ice_render_source_location);
#[cfg(test)]
drop(__ice_render_source);
__ice_rendered
}; __slot_content })();
#[cfg(test)]
drop(__ice_render_source);
__ice_rendered
}); }, ShellTab::Governance => { __children.push({
#[cfg(test)]
let __ice_render_source_location = ::ui_lang_runtime::testing::Location::new("components/shell.ice", 926, 1, "rendered view node");
#[cfg(test)]
let __ice_render_source = ::ui_lang_runtime::testing::push_render_source(__ice_render_source_location);
let __ice_rendered: __IceElement<'_, __DucktapeMessage> = (|| { let __slot_content: __IceElement<'_, __DucktapeMessage> = {
#[cfg(test)]
let __ice_render_source_location = ::ui_lang_runtime::testing::Location::new("view.ice", 244, 1, "rendered view node");
#[cfg(test)]
let __ice_render_source = ::ui_lang_runtime::testing::push_render_source(__ice_render_source_location);
let __ice_rendered: __IceElement<'_, __DucktapeMessage> = { let __ice_node_scope = format!("{}/governance", __ice_node_scope); { let __identified: __IceElement<'_, __DucktapeMessage> = crate::module_view::governance_view((*self.__ice_derived_dark()), self.connected, crate::backend::members_is_admin(::std::convert::AsRef::as_ref(&(self.members_rows)))).map(move |__value| __DucktapeMessage::GovernanceViewEvent(__value)).into(); #[cfg(test)] ::ui_lang_runtime::testing::register_render_source(&__ice_node_scope); ::iced::widget::container(__identified).id(::iced::widget::Id::from(__ice_node_scope.clone())).into() } };
#[cfg(test)]
let __ice_rendered = ::ui_lang_runtime::testing::sourced(__ice_rendered, __ice_render_source_location);
#[cfg(test)]
drop(__ice_render_source);
__ice_rendered
}; __slot_content })();
#[cfg(test)]
drop(__ice_render_source);
__ice_rendered
}); }, ShellTab::Settings => { __children.push({
#[cfg(test)]
let __ice_render_source_location = ::ui_lang_runtime::testing::Location::new("components/shell.ice", 928, 1, "rendered view node");
#[cfg(test)]
let __ice_render_source = ::ui_lang_runtime::testing::push_render_source(__ice_render_source_location);
let __ice_rendered: __IceElement<'_, __DucktapeMessage> = (|| { let __slot_content: __IceElement<'_, __DucktapeMessage> = {
#[cfg(test)]
let __ice_render_source_location = ::ui_lang_runtime::testing::Location::new("view.ice", 260, 1, "rendered view node");
#[cfg(test)]
let __ice_render_source = ::ui_lang_runtime::testing::push_render_source(__ice_render_source_location);
let __ice_rendered: __IceElement<'_, __DucktapeMessage> = { let __ice_node_scope = format!("{}/settings", __ice_node_scope); { let __identified: __IceElement<'_, __DucktapeMessage> = crate::module_view::settings_view((*self.__ice_derived_dark()), self.connected, self.loading, ::std::convert::AsRef::as_ref(&(self.status)), self.mutation_phase.clone(), self.appearance.clone(), self.desktop_notifications, ::std::convert::AsRef::as_ref(&(self.password)), ::std::convert::AsRef::as_ref(&(self.settings_user_key)), ::std::convert::AsRef::as_ref(&(self.account_name)), ::std::convert::AsRef::as_ref(&(self.network_name)), ::std::convert::AsRef::as_ref(&(self.connected_rpc)), ::std::convert::AsRef::as_ref(&(self.account_ceremony_phase)), ::std::convert::AsRef::as_ref(&(self.account_ceremony_qr)), ::std::convert::AsRef::as_ref(&(self.account_ceremony_detail)), ::std::convert::AsRef::as_ref(&(self.account_ceremony_left)), ::std::convert::AsRef::as_ref(&(self.settings_key_state)), ::std::convert::AsRef::as_ref(&(self.settings_key_path)), ::std::convert::AsRef::as_ref(&(self.account_number)), self.account_exists, self.account_busy, ::std::convert::AsRef::as_ref(&(self.account_ticket))).map(move |__value| __DucktapeMessage::SettingsViewEvent(__value)).into(); #[cfg(test)] ::ui_lang_runtime::testing::register_render_source(&__ice_node_scope); ::iced::widget::container(__identified).id(::iced::widget::Id::from(__ice_node_scope.clone())).into() } };
#[cfg(test)]
let __ice_rendered = ::ui_lang_runtime::testing::sourced(__ice_rendered, __ice_render_source_location);
#[cfg(test)]
drop(__ice_render_source);
__ice_rendered
}; __slot_content })();
#[cfg(test)]
drop(__ice_render_source);
__ice_rendered
}); }, } let __child_count = __children.len(); let __children = __children.into_iter().map(|__child| ::ui_lang_runtime::bounded_fill_element(__child, __child_count, false)).collect::<::std::vec::Vec<_>>(); let __layout = ::iced::widget::column(__children).width(::iced::Fill).height(::iced::Fill); let __content = ::iced::widget::container(__layout); let __layout_content: __IceElement<'_, __DucktapeMessage> = __content.into(); let __a11y_key = format!("{}/@layout:904", __ice_use_scope); ::ui_lang_runtime::accessible(__layout_content, ::ui_lang_runtime::StableId::new(&__a11y_key), ::ui_lang_runtime::Role::GenericContainer).logical_id_maybe(::core::cfg!(test).then_some(__a11y_key)).into() };
#[cfg(test)]
drop(__ice_render_source);
__ice_rendered
}; let __container = {  ::iced::widget::container(__container_content).id(::iced::widget::Id::from(__ice_node_scope.clone())).width(::iced::Fill).height(::iced::Fill).clip(true).style(move |__theme| { let mut __style = ::iced::widget::container::Style::default(); __style.background = ::std::option::Option::Some(::iced::Background::Color(__ice_palette.colors[2])); __style }) }; ::ui_lang_runtime::accessible(__container, ::ui_lang_runtime::StableId::new(&__a11y_key), ::ui_lang_runtime::Role::GenericContainer).logical_id_maybe(::core::cfg!(test).then_some(__a11y_key)).into() } };
#[cfg(test)]
drop(__ice_render_source);
__ice_rendered
}); let __child_count = __children.len(); let __children = __children.into_iter().map(|__child| ::ui_lang_runtime::bounded_fill_element(__child, __child_count, true)).collect::<::std::vec::Vec<_>>(); let __layout = ::iced::widget::row(__children).width(::iced::Fill).height(::iced::Fill); let __content = ::iced::widget::container(__layout); let __layout_content: __IceElement<'_, __DucktapeMessage> = __content.into(); let __a11y_key = format!("{}/@layout:883", __ice_use_scope); ::ui_lang_runtime::accessible(__layout_content, ::ui_lang_runtime::StableId::new(&__a11y_key), ::ui_lang_runtime::Role::GenericContainer).logical_id_maybe(::core::cfg!(test).then_some(__a11y_key)).into() };
#[cfg(test)]
drop(__ice_render_source);
__ice_rendered
}); let __child_count = __children.len(); let __children = __children.into_iter().map(|__child| ::ui_lang_runtime::bounded_fill_element(__child, __child_count, false)).collect::<::std::vec::Vec<_>>(); let __layout = ::iced::widget::column(__children).width(::iced::Fill).height(::iced::Fill); let __content = ::iced::widget::container(__layout); let __layout_content: __IceElement<'_, __DucktapeMessage> = __content.into(); let __a11y_key = format!("{}/@layout:863", __ice_use_scope); ::ui_lang_runtime::accessible(__layout_content, ::ui_lang_runtime::StableId::new(&__a11y_key), ::ui_lang_runtime::Role::GenericContainer).logical_id_maybe(::core::cfg!(test).then_some(__a11y_key)).into() };
#[cfg(test)]
drop(__ice_render_source);
__ice_rendered
}); __children.push({
#[cfg(test)]
let __ice_render_source_location = ::ui_lang_runtime::testing::Location::new("components/shell.ice", 930, 1, "rendered view node");
#[cfg(test)]
let __ice_render_source = ::ui_lang_runtime::testing::push_render_source(__ice_render_source_location);
let __ice_rendered: __IceElement<'_, __DucktapeMessage> = (|| { let __slot_content: __IceElement<'_, __DucktapeMessage> = {
#[cfg(test)]
let __ice_render_source_location = ::ui_lang_runtime::testing::Location::new("view.ice", 270, 1, "rendered view node");
#[cfg(test)]
let __ice_render_source = ::ui_lang_runtime::testing::push_render_source(__ice_render_source_location);
let __ice_rendered: __IceElement<'_, __DucktapeMessage> = { let __ice_node_scope = format!("{}/overlays", __ice_use_scope); { let __ice_component_04f7665726c61794c61796572_scope_10480 = __ice_node_scope.clone(); ::ui_lang_runtime::rev_memo(1129u64, ([self.__ice_rev[104], self.__ice_rev[113], self.__ice_rev[114], self.__ice_rev[115], self.__ice_rev[116], self.__ice_rev[117], self.__ice_rev[19], self.__ice_rev[43], self.__ice_rev[44], self.__ice_rev[45], self.__ice_rev[8], self.__ice_rev[9], ], __ice_palette.name), ::ui_lang_runtime::grow_stack(|| self.__ice_component_use_91(__ice_palette, __ice_component_04f7665726c61794c61796572_scope_10480))).into() } };
#[cfg(test)]
drop(__ice_render_source);
__ice_rendered
}; __slot_content })();
#[cfg(test)]
drop(__ice_render_source);
__ice_rendered
}); __children.push({
#[cfg(test)]
let __ice_render_source_location = ::ui_lang_runtime::testing::Location::new("components/shell.ice", 931, 1, "rendered view node");
#[cfg(test)]
let __ice_render_source = ::ui_lang_runtime::testing::push_render_source(__ice_render_source_location);
let __ice_rendered: __IceElement<'_, __DucktapeMessage> = (|| { let __slot_content: __IceElement<'_, __DucktapeMessage> = {
#[cfg(test)]
let __ice_render_source_location = ::ui_lang_runtime::testing::Location::new("view.ice", 293, 1, "rendered view node");
#[cfg(test)]
let __ice_render_source = ::ui_lang_runtime::testing::push_render_source(__ice_render_source_location);
let __ice_rendered: __IceElement<'_, __DucktapeMessage> = { let mut __children: ::std::vec::Vec<__IceElement<'_, __DucktapeMessage>> = ::std::vec::Vec::new(); if self.bell_open { __children.push({
#[cfg(test)]
let __ice_render_source_location = ::ui_lang_runtime::testing::Location::new("view.ice", 295, 1, "rendered view node");
#[cfg(test)]
let __ice_render_source = ::ui_lang_runtime::testing::push_render_source(__ice_render_source_location);
let __ice_rendered: __IceElement<'_, __DucktapeMessage> = { let __a11y_key = format!("{}/@button:295", __ice_use_scope); let __a11y_id = ::ui_lang_runtime::StableId::new(&__a11y_key); let __disabled = false; let __activate = __DucktapeMessage::CloseBell; let __button_content: __IceElement<'_, __DucktapeMessage> = {
#[cfg(test)]
let __ice_render_source_location = ::ui_lang_runtime::testing::Location::new("view.ice", 302, 1, "rendered view node");
#[cfg(test)]
let __ice_render_source = ::ui_lang_runtime::testing::push_render_source(__ice_render_source_location);
let __ice_rendered: __IceElement<'_, __DucktapeMessage> = ::iced::widget::space().width(::iced::Fill).height(::iced::Fill).into();
#[cfg(test)]
drop(__ice_render_source);
__ice_rendered
}; let __button = ::iced::widget::button(__button_content).width(::iced::Fill).height(::iced::Fill).padding(0.0 as f32).on_press_maybe(if __disabled { None } else { Some(__activate.clone()) }).style(move |__theme, __status| { let mut __style = ::iced::widget::button::primary(__theme, __status); let __background: Option<::iced::Color> = match __status { ::iced::widget::button::Status::Hovered => Some(__ice_palette.colors[14]), ::iced::widget::button::Status::Pressed => Some(__ice_palette.colors[39]), ::iced::widget::button::Status::Disabled => Some(::iced::Color::from_rgba8(0, 0, 0, 0.000000)), _ => Some(::iced::Color::from_rgba8(0, 0, 0, 0.000000)) }; if let Some(__background) = __background { __style.background = Some(::iced::Background::Color(__background)); } __style.text_color = __ice_palette.colors[4]; __style.border.radius = 7.0.into(); __style.background = ::std::option::Option::Some(::iced::Background::Color(::iced::Color::from_rgba8(0, 0, 0, 0.000000))); __style.border.color = ::iced::Color::from_rgba8(0, 0, 0, 0.000000); if matches!(__status, ::iced::widget::button::Status::Disabled) { if let Some(::iced::Background::Color(mut __color)) = __style.background { __color.a *= 0.5; __style.background = Some(::iced::Background::Color(__color)); } __style.text_color.a *= 0.5; } __style }); ::ui_lang_runtime::accessible(__button, __a11y_id, ::ui_lang_runtime::Role::Button).logical_id_maybe(::core::cfg!(test).then_some(&*__a11y_key)).focus_id(::iced::widget::Id::from(__a11y_key)).label("Close notifications".to_owned()).disabled(__disabled).on_activate_maybe(if __disabled { None } else { Some(__activate) }).focus_ring(__ice_palette.colors[42], 7.0).into() };
#[cfg(test)]
drop(__ice_render_source);
__ice_rendered
}); } if self.bell_open { __children.push({
#[cfg(test)]
let __ice_render_source_location = ::ui_lang_runtime::testing::Location::new("view.ice", 305, 1, "rendered view node");
#[cfg(test)]
let __ice_render_source = ::ui_lang_runtime::testing::push_render_source(__ice_render_source_location);
let __ice_rendered: __IceElement<'_, __DucktapeMessage> = { let __a11y_key = format!("{}/@container:305", __ice_use_scope); let __container_content: __IceElement<'_, __DucktapeMessage> = {
#[cfg(test)]
let __ice_render_source_location = ::ui_lang_runtime::testing::Location::new("view.ice", 313, 1, "rendered view node");
#[cfg(test)]
let __ice_render_source = ::ui_lang_runtime::testing::push_render_source(__ice_render_source_location);
let __ice_rendered: __IceElement<'_, __DucktapeMessage> = { let __a11y_key = format!("{}/@container:313", __ice_use_scope); let __container_content: __IceElement<'_, __DucktapeMessage> = {
#[cfg(test)]
let __ice_render_source_location = ::ui_lang_runtime::testing::Location::new("view.ice", 324, 1, "rendered view node");
#[cfg(test)]
let __ice_render_source = ::ui_lang_runtime::testing::push_render_source(__ice_render_source_location);
let __ice_rendered: __IceElement<'_, __DucktapeMessage> = { let mut __children: ::std::vec::Vec<__IceElement<'_, __DucktapeMessage>> = ::std::vec::Vec::new(); __children.push({
#[cfg(test)]
let __ice_render_source_location = ::ui_lang_runtime::testing::Location::new("view.ice", 325, 1, "rendered view node");
#[cfg(test)]
let __ice_render_source = ::ui_lang_runtime::testing::push_render_source(__ice_render_source_location);
let __ice_rendered: __IceElement<'_, __DucktapeMessage> = { let __a11y_key = format!("{}/@container:325", __ice_use_scope); let __container_content: __IceElement<'_, __DucktapeMessage> = {
#[cfg(test)]
let __ice_render_source_location = ::ui_lang_runtime::testing::Location::new("view.ice", 332, 1, "rendered view node");
#[cfg(test)]
let __ice_render_source = ::ui_lang_runtime::testing::push_render_source(__ice_render_source_location);
let __ice_rendered: __IceElement<'_, __DucktapeMessage> = { let mut __children: ::std::vec::Vec<__IceElement<'_, __DucktapeMessage>> = ::std::vec::Vec::new(); __children.push({
#[cfg(test)]
let __ice_render_source_location = ::ui_lang_runtime::testing::Location::new("view.ice", 337, 1, "rendered view node");
#[cfg(test)]
let __ice_render_source = ::ui_lang_runtime::testing::push_render_source(__ice_render_source_location);
let __ice_rendered: __IceElement<'_, __DucktapeMessage> = { let __a11y_key = format!("{}/@text:337", __ice_use_scope); let __text_value = ("Alerts").to_string(); let __text = ::iced::widget::text(__text_value.clone()).size(((12.5) as f32).max(f32::EPSILON).min(f32::MAX)).wrapping(::iced::widget::text::Wrapping::None).color(__ice_palette.colors[7]); let __text = ::ui_lang_runtime::selectable_text(__text); ::ui_lang_runtime::accessible(__text, ::ui_lang_runtime::StableId::new(&__a11y_key), ::ui_lang_runtime::Role::Label).logical_id_maybe(::core::cfg!(test).then_some(__a11y_key)).value(__text_value).into() };
#[cfg(test)]
drop(__ice_render_source);
__ice_rendered
}); if (self.bell_unread > 0) { __children.push({
#[cfg(test)]
let __ice_render_source_location = ::ui_lang_runtime::testing::Location::new("view.ice", 347, 1, "rendered view node");
#[cfg(test)]
let __ice_render_source = ::ui_lang_runtime::testing::push_render_source(__ice_render_source_location);
let __ice_rendered: __IceElement<'_, __DucktapeMessage> = { let __a11y_key = format!("{}/@text:347", __ice_use_scope); let __text_value = (crate::backend::count_label(self.bell_unread)).to_string(); let __text = ::iced::widget::text(__text_value.clone()).size(((10.5) as f32).max(f32::EPSILON).min(f32::MAX)).wrapping(::iced::widget::text::Wrapping::None).font(::iced::Font { family: ::iced::font::Family::Name("Geist Mono"), weight: ::iced::font::Weight::Medium, stretch: ::iced::font::Stretch::Normal, style: ::iced::font::Style::Normal }).color(__ice_palette.colors[71]); let __text = ::ui_lang_runtime::selectable_text(__text); ::ui_lang_runtime::accessible(__text, ::ui_lang_runtime::StableId::new(&__a11y_key), ::ui_lang_runtime::Role::Label).logical_id_maybe(::core::cfg!(test).then_some(__a11y_key)).value(__text_value).into() };
#[cfg(test)]
drop(__ice_render_source);
__ice_rendered
}); } if (self.bell_unread > 0) { __children.push({
#[cfg(test)]
let __ice_render_source_location = ::ui_lang_runtime::testing::Location::new("view.ice", 354, 1, "rendered view node");
#[cfg(test)]
let __ice_render_source = ::ui_lang_runtime::testing::push_render_source(__ice_render_source_location);
let __ice_rendered: __IceElement<'_, __DucktapeMessage> = { let __a11y_key = format!("{}/@text:354", __ice_use_scope); let __text_value = ("unread").to_string(); let __text = ::iced::widget::text(__text_value.clone()).size(((12.5) as f32).max(f32::EPSILON).min(f32::MAX)).wrapping(::iced::widget::text::Wrapping::None).color(__ice_palette.colors[71]); let __text = ::ui_lang_runtime::selectable_text(__text); ::ui_lang_runtime::accessible(__text, ::ui_lang_runtime::StableId::new(&__a11y_key), ::ui_lang_runtime::Role::Label).logical_id_maybe(::core::cfg!(test).then_some(__a11y_key)).value(__text_value).into() };
#[cfg(test)]
drop(__ice_render_source);
__ice_rendered
}); } __children.push({
#[cfg(test)]
let __ice_render_source_location = ::ui_lang_runtime::testing::Location::new("view.ice", 359, 1, "rendered view node");
#[cfg(test)]
let __ice_render_source = ::ui_lang_runtime::testing::push_render_source(__ice_render_source_location);
let __ice_rendered: __IceElement<'_, __DucktapeMessage> = ::iced::widget::space().width(::iced::Fill).into();
#[cfg(test)]
drop(__ice_render_source);
__ice_rendered
}); __children.push({
#[cfg(test)]
let __ice_render_source_location = ::ui_lang_runtime::testing::Location::new("view.ice", 360, 1, "rendered view node");
#[cfg(test)]
let __ice_render_source = ::ui_lang_runtime::testing::push_render_source(__ice_render_source_location);
let __ice_rendered: __IceElement<'_, __DucktapeMessage> = { let __ice_node_scope = format!("{}/mark-bell-read", __ice_use_scope); { let __a11y_key = __ice_node_scope.clone(); let __a11y_id = ::ui_lang_runtime::StableId::new(&__a11y_key); let __disabled = ((self.bell_unread <= 0) || self.bell_marking); let __activate = __DucktapeMessage::MarkBellReadSubmit; let __button_content: __IceElement<'_, __DucktapeMessage> = ::iced::widget::text("Mark all read").size(11).line_height(::iced::widget::text::LineHeight::Relative(1.35)).font(::iced::Font { weight: ::iced::font::Weight::Medium, ..Self::default_font() }).into(); let __button = ::iced::widget::button(__button_content).padding(::iced::Padding { top: 7.0, right: 12.0, bottom: 7.0, left: 12.0 }).padding(4.0 as f32).on_press_maybe(if __disabled { None } else { Some(__activate.clone()) }).style(move |__theme, __status| { let mut __style = ::iced::widget::button::primary(__theme, __status); let __background: Option<::iced::Color> = match __status { ::iced::widget::button::Status::Hovered => Some(__ice_palette.colors[14]), ::iced::widget::button::Status::Pressed => Some(__ice_palette.colors[39]), ::iced::widget::button::Status::Disabled => Some(::iced::Color::from_rgba8(0, 0, 0, 0.000000)), _ => Some(::iced::Color::from_rgba8(0, 0, 0, 0.000000)) }; if let Some(__background) = __background { __style.background = Some(::iced::Background::Color(__background)); } __style.text_color = __ice_palette.colors[4]; __style.border.radius = 8.0.into(); __style.background = ::std::option::Option::Some(::iced::Background::Color(::iced::Color::from_rgba8(0, 0, 0, 0.000000))); __style.text_color = __ice_palette.colors[5]; __style.border.color = ::iced::Color::from_rgba8(0, 0, 0, 0.000000); __style.border.width = 1.0 as f32; __style.border.radius = ::iced::border::Radius { top_left: ((6.0) as f32).max(0.0).min(f32::MAX), top_right: ((6.0) as f32).max(0.0).min(f32::MAX), bottom_right: ((6.0) as f32).max(0.0).min(f32::MAX), bottom_left: ((6.0) as f32).max(0.0).min(f32::MAX) }; match __status { ::iced::widget::button::Status::Hovered => { __style.background = ::std::option::Option::Some(::iced::Background::Color(__ice_palette.colors[55])); __style.text_color = __ice_palette.colors[16]; } ::iced::widget::button::Status::Pressed => { __style.background = ::std::option::Option::Some(::iced::Background::Color(__ice_palette.colors[56])); __style.text_color = __ice_palette.colors[16]; } _ => {} } if matches!(__status, ::iced::widget::button::Status::Disabled) { if let Some(::iced::Background::Color(mut __color)) = __style.background { __color.a *= 0.5; __style.background = Some(::iced::Background::Color(__color)); } __style.text_color.a *= 0.5; } __style }); ::ui_lang_runtime::accessible(__button, __a11y_id, ::ui_lang_runtime::Role::Button).logical_id_maybe(::core::cfg!(test).then_some(&*__a11y_key)).focus_id(::iced::widget::Id::from(__a11y_key)).label("Mark all read").disabled(__disabled).on_activate_maybe(if __disabled { None } else { Some(__activate) }).focus_ring(__ice_palette.colors[42], 8.0).into() } };
#[cfg(test)]
drop(__ice_render_source);
__ice_rendered
}); let __child_count = __children.len(); let __children = __children.into_iter().map(|__child| ::ui_lang_runtime::bounded_fill_element(__child, __child_count, true)).collect::<::std::vec::Vec<_>>(); let __layout = ::iced::widget::row(__children).spacing(::ui_lang_runtime::bounded_spacing(8.0, __child_count)).width(::iced::Fill).align_y(::iced::alignment::Vertical::Center); let __content = ::iced::widget::container(__layout); let __layout_content: __IceElement<'_, __DucktapeMessage> = __content.into(); let __a11y_key = format!("{}/@layout:332", __ice_use_scope); ::ui_lang_runtime::accessible(__layout_content, ::ui_lang_runtime::StableId::new(&__a11y_key), ::ui_lang_runtime::Role::GenericContainer).logical_id_maybe(::core::cfg!(test).then_some(__a11y_key)).into() };
#[cfg(test)]
drop(__ice_render_source);
__ice_rendered
}; let __container = {  ::iced::widget::container(__container_content).padding(::ui_lang_runtime::bounded_padding(11.0, 13.0, 9.0, 13.0)).width(::iced::Fill) }; ::ui_lang_runtime::accessible(__container, ::ui_lang_runtime::StableId::new(&__a11y_key), ::ui_lang_runtime::Role::GenericContainer).logical_id_maybe(::core::cfg!(test).then_some(__a11y_key)).into() };
#[cfg(test)]
drop(__ice_render_source);
__ice_rendered
}); __children.push({
#[cfg(test)]
let __ice_render_source_location = ::ui_lang_runtime::testing::Location::new("view.ice", 368, 1, "rendered view node");
#[cfg(test)]
let __ice_render_source = ::ui_lang_runtime::testing::push_render_source(__ice_render_source_location);
let __ice_rendered: __IceElement<'_, __DucktapeMessage> = { let __a11y_key = format!("{}/@container:368", __ice_use_scope); let __container_content: __IceElement<'_, __DucktapeMessage> = {
#[cfg(test)]
let __ice_render_source_location = ::ui_lang_runtime::testing::Location::new("view.ice", 373, 1, "rendered view node");
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
}); if (!(self.bell_error).is_empty()) { __children.push({
#[cfg(test)]
let __ice_render_source_location = ::ui_lang_runtime::testing::Location::new("view.ice", 375, 1, "rendered view node");
#[cfg(test)]
let __ice_render_source = ::ui_lang_runtime::testing::push_render_source(__ice_render_source_location);
let __ice_rendered: __IceElement<'_, __DucktapeMessage> = { let mut __children: ::std::vec::Vec<__IceElement<'_, __DucktapeMessage>> = ::std::vec::Vec::new(); __children.push({
#[cfg(test)]
let __ice_render_source_location = ::ui_lang_runtime::testing::Location::new("view.ice", 376, 1, "rendered view node");
#[cfg(test)]
let __ice_render_source = ::ui_lang_runtime::testing::push_render_source(__ice_render_source_location);
let __ice_rendered: __IceElement<'_, __DucktapeMessage> = { let __a11y_key = format!("{}/@text:376", __ice_use_scope); let __text_value = (self.bell_error).to_string(); let __text = ::iced::widget::text(__text_value.clone()).size(((12.0) as f32).max(f32::EPSILON).min(f32::MAX)).color(__ice_palette.colors[20]); let __text = ::ui_lang_runtime::selectable_text(__text); ::ui_lang_runtime::accessible(__text, ::ui_lang_runtime::StableId::new(&__a11y_key), ::ui_lang_runtime::Role::Label).logical_id_maybe(::core::cfg!(test).then_some(__a11y_key)).value(__text_value).into() };
#[cfg(test)]
drop(__ice_render_source);
__ice_rendered
}); __children.push({
#[cfg(test)]
let __ice_render_source_location = ::ui_lang_runtime::testing::Location::new("view.ice", 377, 1, "rendered view node");
#[cfg(test)]
let __ice_render_source = ::ui_lang_runtime::testing::push_render_source(__ice_render_source_location);
let __ice_rendered: __IceElement<'_, __DucktapeMessage> = { let __a11y_key = format!("{}/@button:377", __ice_use_scope); let __a11y_id = ::ui_lang_runtime::StableId::new(&__a11y_key); let __disabled = false; let __activate = __DucktapeMessage::ReloadBell; let __button_content: __IceElement<'_, __DucktapeMessage> = ::iced::widget::text("Retry").size(12.5).font(::iced::Font { weight: ::iced::font::Weight::Semibold, ..Self::default_font() }).into(); let __button = ::iced::widget::button(__button_content).padding(::iced::Padding { top: 7.0, right: 12.0, bottom: 7.0, left: 12.0 }).on_press_maybe(if __disabled { None } else { Some(__activate.clone()) }).style(move |__theme, __status| { let mut __style = ::iced::widget::button::primary(__theme, __status); let __background: Option<::iced::Color> = match __status { ::iced::widget::button::Status::Hovered => Some(__ice_palette.colors[14]), ::iced::widget::button::Status::Pressed => Some(__ice_palette.colors[39]), ::iced::widget::button::Status::Disabled => Some(::iced::Color::from_rgba8(0, 0, 0, 0.000000)), _ => Some(::iced::Color::from_rgba8(0, 0, 0, 0.000000)) }; if let Some(__background) = __background { __style.background = Some(::iced::Background::Color(__background)); } __style.text_color = __ice_palette.colors[4]; __style.border.radius = 8.0.into(); if matches!(__status, ::iced::widget::button::Status::Disabled) { if let Some(::iced::Background::Color(mut __color)) = __style.background { __color.a *= 0.5; __style.background = Some(::iced::Background::Color(__color)); } __style.text_color.a *= 0.5; } __style }); ::ui_lang_runtime::accessible(__button, __a11y_id, ::ui_lang_runtime::Role::Button).logical_id_maybe(::core::cfg!(test).then_some(&*__a11y_key)).focus_id(::iced::widget::Id::from(__a11y_key)).label("Retry").disabled(__disabled).on_activate_maybe(if __disabled { None } else { Some(__activate) }).focus_ring(__ice_palette.colors[42], 8.0).into() };
#[cfg(test)]
drop(__ice_render_source);
__ice_rendered
}); let __child_count = __children.len(); let __children = __children.into_iter().map(|__child| ::ui_lang_runtime::bounded_fill_element(__child, __child_count, false)).collect::<::std::vec::Vec<_>>(); let __layout = ::iced::widget::column(__children).spacing(::ui_lang_runtime::bounded_spacing(4.0, __child_count)).padding(::ui_lang_runtime::bounded_padding(9.0, 9.0, 9.0, 9.0)); let __content = ::iced::widget::container(__layout); let __layout_content: __IceElement<'_, __DucktapeMessage> = __content.into(); let __a11y_key = format!("{}/@layout:375", __ice_use_scope); ::ui_lang_runtime::accessible(__layout_content, ::ui_lang_runtime::StableId::new(&__a11y_key), ::ui_lang_runtime::Role::GenericContainer).logical_id_maybe(::core::cfg!(test).then_some(__a11y_key)).into() };
#[cfg(test)]
drop(__ice_render_source);
__ice_rendered
}); } if (crate::backend::bell_visible_items(::std::convert::AsRef::as_ref(&(self.bell_items)), ::std::convert::AsRef::as_ref(&(self.account_number)), ::std::convert::AsRef::as_ref(&(self.settings_user_key)))).is_empty() { __children.push({
#[cfg(test)]
let __ice_render_source_location = ::ui_lang_runtime::testing::Location::new("view.ice", 379, 1, "rendered view node");
#[cfg(test)]
let __ice_render_source = ::ui_lang_runtime::testing::push_render_source(__ice_render_source_location);
let __ice_rendered: __IceElement<'_, __DucktapeMessage> = { let __a11y_key = format!("{}/@container:379", __ice_use_scope); let __container_content: __IceElement<'_, __DucktapeMessage> = {
#[cfg(test)]
let __ice_render_source_location = ::ui_lang_runtime::testing::Location::new("view.ice", 384, 1, "rendered view node");
#[cfg(test)]
let __ice_render_source = ::ui_lang_runtime::testing::push_render_source(__ice_render_source_location);
let __ice_rendered: __IceElement<'_, __DucktapeMessage> = { let __a11y_key = format!("{}/@text:384", __ice_use_scope); let __text_value = ("Nothing yet — mentions and deliveries land here.").to_string(); let __text = ::iced::widget::text(__text_value.clone()).size(((12.0) as f32).max(f32::EPSILON).min(f32::MAX)).color(__ice_palette.colors[71]); let __text = ::ui_lang_runtime::selectable_text(__text); ::ui_lang_runtime::accessible(__text, ::ui_lang_runtime::StableId::new(&__a11y_key), ::ui_lang_runtime::Role::Label).logical_id_maybe(::core::cfg!(test).then_some(__a11y_key)).value(__text_value).into() };
#[cfg(test)]
drop(__ice_render_source);
__ice_rendered
}; let __container = {  ::iced::widget::container(__container_content).padding(::ui_lang_runtime::bounded_padding(26.0, 26.0, 26.0, 26.0)).width(::iced::Fill).align_x(::iced::alignment::Horizontal::Center) }; ::ui_lang_runtime::accessible(__container, ::ui_lang_runtime::StableId::new(&__a11y_key), ::ui_lang_runtime::Role::GenericContainer).logical_id_maybe(::core::cfg!(test).then_some(__a11y_key)).into() };
#[cfg(test)]
drop(__ice_render_source);
__ice_rendered
}); } if (!(crate::backend::bell_visible_items(::std::convert::AsRef::as_ref(&(self.bell_items)), ::std::convert::AsRef::as_ref(&(self.account_number)), ::std::convert::AsRef::as_ref(&(self.settings_user_key)))).is_empty()) { __children.push({
#[cfg(test)]
let __ice_render_source_location = ::ui_lang_runtime::testing::Location::new("view.ice", 386, 1, "rendered view node");
#[cfg(test)]
let __ice_render_source = ::ui_lang_runtime::testing::push_render_source(__ice_render_source_location);
let __ice_rendered: __IceElement<'_, __DucktapeMessage> = { let __a11y_key = format!("{}/@layout:386", __ice_use_scope); let __scroll_content: __IceElement<'_, __DucktapeMessage> = {
#[cfg(test)]
let __ice_render_source_location = ::ui_lang_runtime::testing::Location::new("view.ice", 392, 1, "rendered view node");
#[cfg(test)]
let __ice_render_source = ::ui_lang_runtime::testing::push_render_source(__ice_render_source_location);
let __ice_rendered: __IceElement<'_, __DucktapeMessage> = { let mut __children: ::std::vec::Vec<_> = ::std::vec::Vec::new(); for item in crate::backend::bell_visible_items(::std::convert::AsRef::as_ref(&(self.bell_items)), ::std::convert::AsRef::as_ref(&(self.account_number)), ::std::convert::AsRef::as_ref(&(self.settings_user_key))).iter() { let __key = item.seq; let __ice_key_recon = format!("{}/key({})", __ice_use_scope, __key); let _ = &__ice_key_recon; let __child: __IceElement<'_, __DucktapeMessage> = {
#[cfg(test)]
let __ice_render_source_location = ::ui_lang_runtime::testing::Location::new("view.ice", 397, 1, "rendered view node");
#[cfg(test)]
let __ice_render_source = ::ui_lang_runtime::testing::push_render_source(__ice_render_source_location);
let __ice_rendered: __IceElement<'_, __DucktapeMessage> = { let __ice_node_scope = format!("{}/open-notification", format!("{}/key({})", __ice_use_scope, __key)); { let __a11y_key = __ice_node_scope.clone(); let __a11y_id = ::ui_lang_runtime::StableId::new(&__a11y_key); let __disabled = (!crate::backend::bell_openable(::std::borrow::Borrow::borrow(&(item)), ::std::convert::AsRef::as_ref(&(self.bell_presentations)))); let __activate = __DucktapeMessage::BellOpenItem(self.connect_generation, self.account_number.to_owned(), crate::backend::bell_presentation(::std::borrow::Borrow::borrow(&(item)), ::std::convert::AsRef::as_ref(&(self.bell_presentations)))); let __button_content: __IceElement<'_, __DucktapeMessage> = {
#[cfg(test)]
let __ice_render_source_location = ::ui_lang_runtime::testing::Location::new("view.ice", 404, 1, "rendered view node");
#[cfg(test)]
let __ice_render_source = ::ui_lang_runtime::testing::push_render_source(__ice_render_source_location);
let __ice_rendered: __IceElement<'_, __DucktapeMessage> = { let __ice_component_042656c6c526f77_scope_10614 = format!("{}/BellRow@10614", __ice_key_recon); ::ui_lang_runtime::rev_memo(1160u64, (self.__ice_rev[107], self.__ice_rev[108], self.__ice_rev[70], self.__ice_rev[73], __ice_palette.name), ::ui_lang_runtime::grow_stack(|| self.__ice_component_use_95(__ice_palette, __ice_component_042656c6c526f77_scope_10614, item.clone(), crate::backend::bell_presentation(::std::borrow::Borrow::borrow(&(item)), ::std::convert::AsRef::as_ref(&(self.bell_presentations)))))).into() };
#[cfg(test)]
drop(__ice_render_source);
__ice_rendered
}; let __button = ::iced::widget::button(__button_content).padding(::iced::Padding { top: 7.0, right: 12.0, bottom: 7.0, left: 12.0 }).width(::iced::Fill).padding(0.0 as f32).on_press_maybe(if __disabled { None } else { Some(__activate.clone()) }).style(move |__theme, __status| { let mut __style = ::iced::widget::button::primary(__theme, __status); let __background: Option<::iced::Color> = match __status { ::iced::widget::button::Status::Hovered => Some(__ice_palette.colors[14]), ::iced::widget::button::Status::Pressed => Some(__ice_palette.colors[39]), ::iced::widget::button::Status::Disabled => Some(::iced::Color::from_rgba8(0, 0, 0, 0.000000)), _ => Some(::iced::Color::from_rgba8(0, 0, 0, 0.000000)) }; if let Some(__background) = __background { __style.background = Some(::iced::Background::Color(__background)); } __style.text_color = __ice_palette.colors[4]; __style.border.radius = 8.0.into(); if matches!(__status, ::iced::widget::button::Status::Disabled) { if let Some(::iced::Background::Color(mut __color)) = __style.background { __color.a *= 0.5; __style.background = Some(::iced::Background::Color(__color)); } __style.text_color.a *= 0.5; } __style }); ::ui_lang_runtime::accessible(__button, __a11y_id, ::ui_lang_runtime::Role::Button).logical_id_maybe(::core::cfg!(test).then_some(&*__a11y_key)).focus_id(::iced::widget::Id::from(__a11y_key)).label(crate::backend::bell_label(::std::borrow::Borrow::borrow(&(item)), ::std::convert::AsRef::as_ref(&(self.bell_presentations)))).disabled(__disabled).on_activate_maybe(if __disabled { None } else { Some(__activate) }).focus_ring(__ice_palette.colors[42], 8.0).into() } };
#[cfg(test)]
drop(__ice_render_source);
__ice_rendered
}; __children.push((__key, __child)); } let __child_count = __children.len(); let __children = __children.into_iter().map(|(__key, __child)| (__key, ::ui_lang_runtime::bounded_fill_element(__child, __child_count, false))).collect::<::std::vec::Vec<_>>(); let __layout = ::iced::widget::keyed_column(__children).spacing(::ui_lang_runtime::bounded_spacing(1.0, __child_count)).padding(::ui_lang_runtime::bounded_padding(5.0, 5.0, 5.0, 5.0)).width(::iced::Fill); __layout.into() };
#[cfg(test)]
drop(__ice_render_source);
__ice_rendered
}; let __layout = ::ui_lang_runtime::scroll_anchor(::iced::widget::scrollable(__scroll_content).direction(::iced::widget::scrollable::Direction::Vertical(::iced::widget::scrollable::Scrollbar::new())).anchor_x(::iced::widget::scrollable::Anchor::Start).anchor_y(::iced::widget::scrollable::Anchor::Start).width(::iced::Fill).height(290.0 as f32)); ::ui_lang_runtime::accessible(__layout, ::ui_lang_runtime::StableId::new(&__a11y_key), ::ui_lang_runtime::Role::GenericContainer).logical_id_maybe(::core::cfg!(test).then_some(__a11y_key)).into() };
#[cfg(test)]
drop(__ice_render_source);
__ice_rendered
}); } let __child_count = __children.len(); let __children = __children.into_iter().map(|__child| ::ui_lang_runtime::bounded_fill_element(__child, __child_count, false)).collect::<::std::vec::Vec<_>>(); let __layout = ::iced::widget::column(__children).width(::iced::Fill); let __content = ::iced::widget::container(__layout); let __layout_content: __IceElement<'_, __DucktapeMessage> = __content.into(); let __a11y_key = format!("{}/@layout:324", __ice_use_scope); ::ui_lang_runtime::accessible(__layout_content, ::ui_lang_runtime::StableId::new(&__a11y_key), ::ui_lang_runtime::Role::GenericContainer).logical_id_maybe(::core::cfg!(test).then_some(__a11y_key)).into() };
#[cfg(test)]
drop(__ice_render_source);
__ice_rendered
}; let __container = {  ::iced::widget::container(__container_content).width(342.0 as f32).clip(true).style(move |__theme| { let mut __style = ::iced::widget::container::Style::default(); __style.background = ::std::option::Option::Some(::iced::Background::Color(__ice_palette.colors[3])); __style.border.color = __ice_palette.colors[39]; __style.border.width = 1.0 as f32; __style.border.radius = ::iced::border::Radius { top_left: ((13.0) as f32).max(0.0).min(f32::MAX), top_right: ((13.0) as f32).max(0.0).min(f32::MAX), bottom_right: ((13.0) as f32).max(0.0).min(f32::MAX), bottom_left: ((13.0) as f32).max(0.0).min(f32::MAX) }; __style.shadow.color = __ice_palette.colors[48]; __style.shadow.offset.y = 16.0 as f32; __style.shadow.blur_radius = 40.0 as f32; __style }) }; ::ui_lang_runtime::accessible(__container, ::ui_lang_runtime::StableId::new(&__a11y_key), ::ui_lang_runtime::Role::GenericContainer).logical_id_maybe(::core::cfg!(test).then_some(__a11y_key)).into() };
#[cfg(test)]
drop(__ice_render_source);
__ice_rendered
}; let __container = {  ::iced::widget::container(__container_content).padding(::ui_lang_runtime::bounded_padding(44.0, 13.0, 0.0, 0.0)).width(::iced::Fill).height(::iced::Fill).align_x(::iced::alignment::Horizontal::Right).align_y(::iced::alignment::Vertical::Top) }; ::ui_lang_runtime::accessible(__container, ::ui_lang_runtime::StableId::new(&__a11y_key), ::ui_lang_runtime::Role::GenericContainer).logical_id_maybe(::core::cfg!(test).then_some(__a11y_key)).into() };
#[cfg(test)]
drop(__ice_render_source);
__ice_rendered
}); } let __layout = ::ui_lang_runtime::zstack(__children).width(::iced::Fill).height(::iced::Fill); let __content = ::iced::widget::container(__layout); let __layout_content: __IceElement<'_, __DucktapeMessage> = __content.into(); let __a11y_key = format!("{}/@layout:293", __ice_use_scope); ::ui_lang_runtime::accessible(__layout_content, ::ui_lang_runtime::StableId::new(&__a11y_key), ::ui_lang_runtime::Role::GenericContainer).logical_id_maybe(::core::cfg!(test).then_some(__a11y_key)).into() };
#[cfg(test)]
drop(__ice_render_source);
__ice_rendered
}; __slot_content })();
#[cfg(test)]
drop(__ice_render_source);
__ice_rendered
}); let __layout = ::ui_lang_runtime::zstack(__children).width(::iced::Fill).height(::iced::Fill); let __content = ::iced::widget::container(__layout); let __layout_content: __IceElement<'_, __DucktapeMessage> = __content.into(); let __a11y_key = format!("{}/@layout:862", __ice_use_scope); ::ui_lang_runtime::accessible(__layout_content, ::ui_lang_runtime::StableId::new(&__a11y_key), ::ui_lang_runtime::Role::GenericContainer).logical_id_maybe(::core::cfg!(test).then_some(__a11y_key)).into() };
#[cfg(test)]
drop(__ice_render_source);
__ice_rendered
}; let __container = {  ::iced::widget::container(__container_content).width(::iced::Fill).height(::iced::Fill).clip(true).style(move |__theme| { let mut __style = ::iced::widget::container::Style::default(); __style.background = ::std::option::Option::Some(::iced::Background::Color(__ice_palette.colors[2])); __style.snap = true; __style }) }; ::ui_lang_runtime::accessible(__container, ::ui_lang_runtime::StableId::new(&__a11y_key), ::ui_lang_runtime::Role::GenericContainer).logical_id_maybe(::core::cfg!(test).then_some(__a11y_key)).into() };
#[cfg(test)]
drop(__ice_render_source);
__ice_rendered
}; __component_content }
#[cfg(test)]
pub(super) fn __ice_component_use_97(&self, __ice_palette: __IcePalette, __ice_use_scope: ::std::string::String, __ice_cb_0: impl Fn() -> __DucktapeMessage + Clone + 'static) -> __IceElement<'_, __DucktapeMessage> { let __component_content: __IceElement<'_, __DucktapeMessage> = {
#[cfg(test)]
let __ice_render_source_location = ::ui_lang_runtime::testing::Location::new("components/shell.ice", 41, 1, "rendered view node");
#[cfg(test)]
let __ice_render_source = ::ui_lang_runtime::testing::push_render_source(__ice_render_source_location);
let __ice_rendered: __IceElement<'_, __DucktapeMessage> = { let __ice_node_scope = format!("{}/root", __ice_use_scope); { let __a11y_key = __ice_node_scope.clone(); let __a11y_id = ::ui_lang_runtime::StableId::new(&__a11y_key); let __disabled = false; let __activate = (__ice_cb_0)(); let __button_content: __IceElement<'_, __DucktapeMessage> = {
#[cfg(test)]
let __ice_render_source_location = ::ui_lang_runtime::testing::Location::new("components/shell.ice", 46, 1, "rendered view node");
#[cfg(test)]
let __ice_render_source = ::ui_lang_runtime::testing::push_render_source(__ice_render_source_location);
let __ice_rendered: __IceElement<'_, __DucktapeMessage> = { let mut __children: ::std::vec::Vec<__IceElement<'_, __DucktapeMessage>> = ::std::vec::Vec::new(); __children.push({
#[cfg(test)]
let __ice_render_source_location = ::ui_lang_runtime::testing::Location::new("components/shell.ice", 47, 1, "rendered view node");
#[cfg(test)]
let __ice_render_source = ::ui_lang_runtime::testing::push_render_source(__ice_render_source_location);
let __ice_rendered: __IceElement<'_, __DucktapeMessage> = { let __a11y_key = format!("{}/@container:47", __ice_use_scope); let __container_content: __IceElement<'_, __DucktapeMessage> = {
#[cfg(test)]
let __ice_render_source_location = ::ui_lang_runtime::testing::Location::new("components/shell.ice", 55, 1, "rendered view node");
#[cfg(test)]
let __ice_render_source = ::ui_lang_runtime::testing::push_render_source(__ice_render_source_location);
let __ice_rendered: __IceElement<'_, __DucktapeMessage> = { let __a11y_key = format!("{}/@text:55", __ice_use_scope); let __text_value = ("◆").to_string(); let __text = ::iced::widget::text(__text_value.clone()).size(((7.5) as f32).max(f32::EPSILON).min(f32::MAX)).wrapping(::iced::widget::text::Wrapping::None).font(::iced::Font { family: ::iced::font::Family::Name("Geist Mono"), weight: ::iced::font::Weight::Semibold, stretch: ::iced::font::Stretch::Normal, style: ::iced::font::Style::Normal }).color(__ice_palette.colors[38]); let __text = ::ui_lang_runtime::selectable_text(__text); ::ui_lang_runtime::accessible(__text, ::ui_lang_runtime::StableId::new(&__a11y_key), ::ui_lang_runtime::Role::Label).logical_id_maybe(::core::cfg!(test).then_some(__a11y_key)).value(__text_value).into() };
#[cfg(test)]
drop(__ice_render_source);
__ice_rendered
}; let __container = {  ::iced::widget::container(__container_content).width(15.0 as f32).height(15.0 as f32).align_x(::iced::alignment::Horizontal::Center).align_y(::iced::alignment::Vertical::Center).style(move |__theme| { let mut __style = ::iced::widget::container::Style::default(); __style.background = ::std::option::Option::Some(::iced::Background::Color(__ice_palette.colors[7])); __style.border.radius = ::iced::border::Radius { top_left: ((4.0) as f32).max(0.0).min(f32::MAX), top_right: ((4.0) as f32).max(0.0).min(f32::MAX), bottom_right: ((4.0) as f32).max(0.0).min(f32::MAX), bottom_left: ((4.0) as f32).max(0.0).min(f32::MAX) }; __style }) }; ::ui_lang_runtime::accessible(__container, ::ui_lang_runtime::StableId::new(&__a11y_key), ::ui_lang_runtime::Role::GenericContainer).logical_id_maybe(::core::cfg!(test).then_some(__a11y_key)).into() };
#[cfg(test)]
drop(__ice_render_source);
__ice_rendered
}); __children.push({
#[cfg(test)]
let __ice_render_source_location = ::ui_lang_runtime::testing::Location::new("components/shell.ice", 61, 1, "rendered view node");
#[cfg(test)]
let __ice_render_source = ::ui_lang_runtime::testing::push_render_source(__ice_render_source_location);
let __ice_rendered: __IceElement<'_, __DucktapeMessage> = { let __a11y_key = format!("{}/@text:61", __ice_use_scope); let __text_value = ("testnet").to_string(); let __text = ::iced::widget::text(__text_value.clone()).size(((11.5) as f32).max(f32::EPSILON).min(f32::MAX)).wrapping(::iced::widget::text::Wrapping::None).font(::iced::Font { family: ::iced::font::Family::Name("Geist"), weight: ::iced::font::Weight::Semibold, stretch: ::iced::font::Stretch::Normal, style: ::iced::font::Style::Normal }).color(__ice_palette.colors[15]); let __text = ::ui_lang_runtime::selectable_text(__text); ::ui_lang_runtime::accessible(__text, ::ui_lang_runtime::StableId::new(&__a11y_key), ::ui_lang_runtime::Role::Label).logical_id_maybe(::core::cfg!(test).then_some(__a11y_key)).value(__text_value).into() };
#[cfg(test)]
drop(__ice_render_source);
__ice_rendered
}); let __child_count = __children.len(); let __children = __children.into_iter().map(|__child| ::ui_lang_runtime::bounded_fill_element(__child, __child_count, true)).collect::<::std::vec::Vec<_>>(); let __layout = ::iced::widget::row(__children).spacing(::ui_lang_runtime::bounded_spacing(7.0, __child_count)).align_y(::iced::alignment::Vertical::Center); let __content = ::iced::widget::container(__layout); let __layout_content: __IceElement<'_, __DucktapeMessage> = __content.into(); let __a11y_key = format!("{}/@layout:46", __ice_use_scope); ::ui_lang_runtime::accessible(__layout_content, ::ui_lang_runtime::StableId::new(&__a11y_key), ::ui_lang_runtime::Role::GenericContainer).logical_id_maybe(::core::cfg!(test).then_some(__a11y_key)).into() };
#[cfg(test)]
drop(__ice_render_source);
__ice_rendered
}; let __button = ::iced::widget::button(__button_content).padding(::iced::Padding { top: 7.0, right: 12.0, bottom: 7.0, left: 12.0 }).padding(4.0 as f32).on_press_maybe(if __disabled { None } else { Some(__activate.clone()) }).style(move |__theme, __status| { let mut __style = ::iced::widget::button::primary(__theme, __status); let __background: Option<::iced::Color> = match __status { ::iced::widget::button::Status::Hovered => Some(__ice_palette.colors[14]), ::iced::widget::button::Status::Pressed => Some(__ice_palette.colors[39]), ::iced::widget::button::Status::Disabled => Some(::iced::Color::from_rgba8(0, 0, 0, 0.000000)), _ => Some(::iced::Color::from_rgba8(0, 0, 0, 0.000000)) }; if let Some(__background) = __background { __style.background = Some(::iced::Background::Color(__background)); } __style.text_color = __ice_palette.colors[4]; __style.border.radius = 8.0.into(); __style.background = ::std::option::Option::Some(::iced::Background::Color(::iced::Color::from_rgba8(0, 0, 0, 0.000000))); __style.text_color = __ice_palette.colors[5]; __style.border.color = ::iced::Color::from_rgba8(0, 0, 0, 0.000000); __style.border.width = 1.0 as f32; __style.border.radius = ::iced::border::Radius { top_left: ((7.0) as f32).max(0.0).min(f32::MAX), top_right: ((7.0) as f32).max(0.0).min(f32::MAX), bottom_right: ((7.0) as f32).max(0.0).min(f32::MAX), bottom_left: ((7.0) as f32).max(0.0).min(f32::MAX) }; match __status { ::iced::widget::button::Status::Hovered => { __style.background = ::std::option::Option::Some(::iced::Background::Color(__ice_palette.colors[58])); __style.text_color = __ice_palette.colors[4]; } ::iced::widget::button::Status::Pressed => { __style.background = ::std::option::Option::Some(::iced::Background::Color(__ice_palette.colors[56])); __style.text_color = __ice_palette.colors[4]; } _ => {} } if matches!(__status, ::iced::widget::button::Status::Disabled) { if let Some(::iced::Background::Color(mut __color)) = __style.background { __color.a *= 0.5; __style.background = Some(::iced::Background::Color(__color)); } __style.text_color.a *= 0.5; } __style }); ::ui_lang_runtime::accessible(__button, __a11y_id, ::ui_lang_runtime::Role::Button).logical_id_maybe(::core::cfg!(test).then_some(&*__a11y_key)).focus_id(::iced::widget::Id::from(__a11y_key)).label("Switch network".to_owned()).disabled(__disabled).on_activate_maybe(if __disabled { None } else { Some(__activate) }).focus_ring(__ice_palette.colors[42], 8.0).into() } };
#[cfg(test)]
drop(__ice_render_source);
__ice_rendered
}; __component_content }
#[cfg(test)]
pub(super) fn __ice_component_use_98(&self, __ice_palette: __IcePalette, __ice_use_scope: ::std::string::String) -> __IceElement<'_, __DucktapeMessage> { let __component_content: __IceElement<'_, __DucktapeMessage> = {
#[cfg(test)]
let __ice_render_source_location = ::ui_lang_runtime::testing::Location::new("components/shell.ice", 75, 1, "rendered view node");
#[cfg(test)]
let __ice_render_source = ::ui_lang_runtime::testing::push_render_source(__ice_render_source_location);
let __ice_rendered: __IceElement<'_, __DucktapeMessage> = { let __ice_node_scope = format!("{}/root", __ice_use_scope); { let mut __children: ::std::vec::Vec<__IceElement<'_, __DucktapeMessage>> = ::std::vec::Vec::new(); if ((!false) && self.loading) { __children.push({
#[cfg(test)]
let __ice_render_source_location = ::ui_lang_runtime::testing::Location::new("components/shell.ice", 85, 1, "rendered view node");
#[cfg(test)]
let __ice_render_source = ::ui_lang_runtime::testing::push_render_source(__ice_render_source_location);
let __ice_rendered: __IceElement<'_, __DucktapeMessage> = { let __a11y_key = format!("{}/@container:85", __ice_use_scope); let __container_content: __IceElement<'_, __DucktapeMessage> = {
#[cfg(test)]
let __ice_render_source_location = ::ui_lang_runtime::testing::Location::new("components/shell.ice", 91, 1, "rendered view node");
#[cfg(test)]
let __ice_render_source = ::ui_lang_runtime::testing::push_render_source(__ice_render_source_location);
let __ice_rendered: __IceElement<'_, __DucktapeMessage> = ::iced::widget::space().width(1.0 as f32).height(1.0 as f32).into();
#[cfg(test)]
drop(__ice_render_source);
__ice_rendered
}; let __container = {  ::iced::widget::container(__container_content).width(6.0 as f32).height(6.0 as f32).style(move |__theme| { let mut __style = ::iced::widget::container::Style::default(); __style.background = ::std::option::Option::Some(::iced::Background::Color(__ice_palette.colors[34])); __style.border.radius = ::iced::border::Radius { top_left: (((6.0 / 2.0)) as f32).max(0.0).min(f32::MAX), top_right: (((6.0 / 2.0)) as f32).max(0.0).min(f32::MAX), bottom_right: (((6.0 / 2.0)) as f32).max(0.0).min(f32::MAX), bottom_left: (((6.0 / 2.0)) as f32).max(0.0).min(f32::MAX) }; __style }) }; ::ui_lang_runtime::accessible(__container, ::ui_lang_runtime::StableId::new(&__a11y_key), ::ui_lang_runtime::Role::GenericContainer).logical_id_maybe(::core::cfg!(test).then_some(__a11y_key)).into() };
#[cfg(test)]
drop(__ice_render_source);
__ice_rendered
}); } if ((!false) && (!self.loading)) { __children.push({
#[cfg(test)]
let __ice_render_source_location = ::ui_lang_runtime::testing::Location::new("components/shell.ice", 93, 1, "rendered view node");
#[cfg(test)]
let __ice_render_source = ::ui_lang_runtime::testing::push_render_source(__ice_render_source_location);
let __ice_rendered: __IceElement<'_, __DucktapeMessage> = { let __a11y_key = format!("{}/@container:93", __ice_use_scope); let __container_content: __IceElement<'_, __DucktapeMessage> = {
#[cfg(test)]
let __ice_render_source_location = ::ui_lang_runtime::testing::Location::new("components/shell.ice", 99, 1, "rendered view node");
#[cfg(test)]
let __ice_render_source = ::ui_lang_runtime::testing::push_render_source(__ice_render_source_location);
let __ice_rendered: __IceElement<'_, __DucktapeMessage> = ::iced::widget::space().width(1.0 as f32).height(1.0 as f32).into();
#[cfg(test)]
drop(__ice_render_source);
__ice_rendered
}; let __container = {  ::iced::widget::container(__container_content).width(6.0 as f32).height(6.0 as f32).style(move |__theme| { let mut __style = ::iced::widget::container::Style::default(); __style.background = ::std::option::Option::Some(::iced::Background::Color(__ice_palette.colors[29])); __style.border.radius = ::iced::border::Radius { top_left: (((6.0 / 2.0)) as f32).max(0.0).min(f32::MAX), top_right: (((6.0 / 2.0)) as f32).max(0.0).min(f32::MAX), bottom_right: (((6.0 / 2.0)) as f32).max(0.0).min(f32::MAX), bottom_left: (((6.0 / 2.0)) as f32).max(0.0).min(f32::MAX) }; __style }) }; ::ui_lang_runtime::accessible(__container, ::ui_lang_runtime::StableId::new(&__a11y_key), ::ui_lang_runtime::Role::GenericContainer).logical_id_maybe(::core::cfg!(test).then_some(__a11y_key)).into() };
#[cfg(test)]
drop(__ice_render_source);
__ice_rendered
}); } let __child_count = __children.len(); let __children = __children.into_iter().map(|__child| ::ui_lang_runtime::bounded_fill_element(__child, __child_count, false)).collect::<::std::vec::Vec<_>>(); let __layout = ::iced::widget::column(__children); let __content = ::iced::widget::container(__layout); let __layout_content: __IceElement<'_, __DucktapeMessage> = __content.into(); let __a11y_key = __ice_node_scope.as_str(); ::ui_lang_runtime::accessible(__layout_content, ::ui_lang_runtime::StableId::new(&__a11y_key), ::ui_lang_runtime::Role::GenericContainer).logical_id_maybe(::core::cfg!(test).then_some(__a11y_key)).into() } };
#[cfg(test)]
drop(__ice_render_source);
__ice_rendered
}; __component_content }
#[cfg(test)]
pub(super) fn __ice_component_use_99(&self, __ice_palette: __IcePalette, __ice_use_scope: ::std::string::String) -> __IceElement<'_, __DucktapeMessage> { let __component_content: __IceElement<'_, __DucktapeMessage> = {
#[cfg(test)]
let __ice_render_source_location = ::ui_lang_runtime::testing::Location::new("components/shell.ice", 104, 1, "rendered view node");
#[cfg(test)]
let __ice_render_source = ::ui_lang_runtime::testing::push_render_source(__ice_render_source_location);
let __ice_rendered: __IceElement<'_, __DucktapeMessage> = { let __ice_node_scope = format!("{}/root", __ice_use_scope); { let __a11y_key = __ice_node_scope.as_str(); let __container_content: __IceElement<'_, __DucktapeMessage> = {
#[cfg(test)]
let __ice_render_source_location = ::ui_lang_runtime::testing::Location::new("components/shell.ice", 112, 1, "rendered view node");
#[cfg(test)]
let __ice_render_source = ::ui_lang_runtime::testing::push_render_source(__ice_render_source_location);
let __ice_rendered: __IceElement<'_, __DucktapeMessage> = { let mut __children: ::std::vec::Vec<__IceElement<'_, __DucktapeMessage>> = ::std::vec::Vec::new(); __children.push({
#[cfg(test)]
let __ice_render_source_location = ::ui_lang_runtime::testing::Location::new("components/shell.ice", 113, 1, "rendered view node");
#[cfg(test)]
let __ice_render_source = ::ui_lang_runtime::testing::push_render_source(__ice_render_source_location);
let __ice_rendered: __IceElement<'_, __DucktapeMessage> = { let __ice_component_0537461747573446f74_scope_3033 = format!("{}/StatusDot@3033", __ice_use_scope); ::ui_lang_runtime::rev_memo(313u64, (self.__ice_rev[9], __ice_palette.name), ::ui_lang_runtime::grow_stack(|| self.__ice_component_use_98(__ice_palette, __ice_component_0537461747573446f74_scope_3033))).into() };
#[cfg(test)]
drop(__ice_render_source);
__ice_rendered
}); if ((!false) && self.loading) { __children.push({
#[cfg(test)]
let __ice_render_source_location = ::ui_lang_runtime::testing::Location::new("components/shell.ice", 126, 1, "rendered view node");
#[cfg(test)]
let __ice_render_source = ::ui_lang_runtime::testing::push_render_source(__ice_render_source_location);
let __ice_rendered: __IceElement<'_, __DucktapeMessage> = { let __a11y_key = format!("{}/@text:126", __ice_use_scope); let __text_value = ("Syncing…").to_string(); let __text = ::iced::widget::text(__text_value.clone()).size(((10.5) as f32).max(f32::EPSILON).min(f32::MAX)).wrapping(::iced::widget::text::Wrapping::None).font(::iced::Font { family: ::iced::font::Family::Name("Geist Mono"), weight: ::iced::font::Weight::Medium, stretch: ::iced::font::Stretch::Normal, style: ::iced::font::Style::Normal }).color(__ice_palette.colors[41]); let __text = ::ui_lang_runtime::selectable_text(__text); ::ui_lang_runtime::accessible(__text, ::ui_lang_runtime::StableId::new(&__a11y_key), ::ui_lang_runtime::Role::Label).logical_id_maybe(::core::cfg!(test).then_some(__a11y_key)).value(__text_value).into() };
#[cfg(test)]
drop(__ice_render_source);
__ice_rendered
}); } if ((!false) && (!self.loading)) { __children.push({
#[cfg(test)]
let __ice_render_source_location = ::ui_lang_runtime::testing::Location::new("components/shell.ice", 133, 1, "rendered view node");
#[cfg(test)]
let __ice_render_source = ::ui_lang_runtime::testing::push_render_source(__ice_render_source_location);
let __ice_rendered: __IceElement<'_, __DucktapeMessage> = { let __a11y_key = format!("{}/@text:133", __ice_use_scope); let __text_value = ("Synced").to_string(); let __text = ::iced::widget::text(__text_value.clone()).size(((10.5) as f32).max(f32::EPSILON).min(f32::MAX)).wrapping(::iced::widget::text::Wrapping::None).font(::iced::Font { family: ::iced::font::Family::Name("Geist Mono"), weight: ::iced::font::Weight::Medium, stretch: ::iced::font::Stretch::Normal, style: ::iced::font::Style::Normal }).color(__ice_palette.colors[41]); let __text = ::ui_lang_runtime::selectable_text(__text); ::ui_lang_runtime::accessible(__text, ::ui_lang_runtime::StableId::new(&__a11y_key), ::ui_lang_runtime::Role::Label).logical_id_maybe(::core::cfg!(test).then_some(__a11y_key)).value(__text_value).into() };
#[cfg(test)]
drop(__ice_render_source);
__ice_rendered
}); } let __child_count = __children.len(); let __children = __children.into_iter().map(|__child| ::ui_lang_runtime::bounded_fill_element(__child, __child_count, true)).collect::<::std::vec::Vec<_>>(); let __layout = ::iced::widget::row(__children).spacing(::ui_lang_runtime::bounded_spacing(5.0, __child_count)).align_y(::iced::alignment::Vertical::Center); let __content = ::iced::widget::container(__layout); let __layout_content: __IceElement<'_, __DucktapeMessage> = __content.into(); let __a11y_key = format!("{}/@layout:112", __ice_use_scope); ::ui_lang_runtime::accessible(__layout_content, ::ui_lang_runtime::StableId::new(&__a11y_key), ::ui_lang_runtime::Role::GenericContainer).logical_id_maybe(::core::cfg!(test).then_some(__a11y_key)).into() };
#[cfg(test)]
drop(__ice_render_source);
__ice_rendered
}; let __container = {  ::iced::widget::container(__container_content).id(::iced::widget::Id::from(__ice_node_scope.clone())).padding(::ui_lang_runtime::bounded_padding(3.0, 8.0, 3.0, 8.0)).style(move |__theme| { let mut __style = ::iced::widget::container::Style::default(); __style.background = ::std::option::Option::Some(::iced::Background::Color(__ice_palette.colors[3])); __style.border.color = __ice_palette.colors[39]; __style.border.width = 1.0 as f32; __style.border.radius = ::iced::border::Radius { top_left: ((7.0) as f32).max(0.0).min(f32::MAX), top_right: ((7.0) as f32).max(0.0).min(f32::MAX), bottom_right: ((7.0) as f32).max(0.0).min(f32::MAX), bottom_left: ((7.0) as f32).max(0.0).min(f32::MAX) }; __style }) }; ::ui_lang_runtime::accessible(__container, ::ui_lang_runtime::StableId::new(&__a11y_key), ::ui_lang_runtime::Role::GenericContainer).logical_id_maybe(::core::cfg!(test).then_some(__a11y_key)).into() } };
#[cfg(test)]
drop(__ice_render_source);
__ice_rendered
}; __component_content }
#[cfg(test)]
pub(super) fn __ice_component_use_100(&self, __ice_palette: __IcePalette, __ice_use_scope: ::std::string::String) -> __IceElement<'_, __DucktapeMessage> { let __component_content: __IceElement<'_, __DucktapeMessage> = {
#[cfg(test)]
let __ice_render_source_location = ::ui_lang_runtime::testing::Location::new("components/shell.ice", 75, 1, "rendered view node");
#[cfg(test)]
let __ice_render_source = ::ui_lang_runtime::testing::push_render_source(__ice_render_source_location);
let __ice_rendered: __IceElement<'_, __DucktapeMessage> = { let __ice_node_scope = format!("{}/root", __ice_use_scope); { let mut __children: ::std::vec::Vec<__IceElement<'_, __DucktapeMessage>> = ::std::vec::Vec::new(); if ((!false) && self.loading) { __children.push({
#[cfg(test)]
let __ice_render_source_location = ::ui_lang_runtime::testing::Location::new("components/shell.ice", 85, 1, "rendered view node");
#[cfg(test)]
let __ice_render_source = ::ui_lang_runtime::testing::push_render_source(__ice_render_source_location);
let __ice_rendered: __IceElement<'_, __DucktapeMessage> = { let __a11y_key = format!("{}/@container:85", __ice_use_scope); let __container_content: __IceElement<'_, __DucktapeMessage> = {
#[cfg(test)]
let __ice_render_source_location = ::ui_lang_runtime::testing::Location::new("components/shell.ice", 91, 1, "rendered view node");
#[cfg(test)]
let __ice_render_source = ::ui_lang_runtime::testing::push_render_source(__ice_render_source_location);
let __ice_rendered: __IceElement<'_, __DucktapeMessage> = ::iced::widget::space().width(1.0 as f32).height(1.0 as f32).into();
#[cfg(test)]
drop(__ice_render_source);
__ice_rendered
}; let __container = {  ::iced::widget::container(__container_content).width(7.0 as f32).height(7.0 as f32).style(move |__theme| { let mut __style = ::iced::widget::container::Style::default(); __style.background = ::std::option::Option::Some(::iced::Background::Color(__ice_palette.colors[34])); __style.border.radius = ::iced::border::Radius { top_left: (((7.0 / 2.0)) as f32).max(0.0).min(f32::MAX), top_right: (((7.0 / 2.0)) as f32).max(0.0).min(f32::MAX), bottom_right: (((7.0 / 2.0)) as f32).max(0.0).min(f32::MAX), bottom_left: (((7.0 / 2.0)) as f32).max(0.0).min(f32::MAX) }; __style }) }; ::ui_lang_runtime::accessible(__container, ::ui_lang_runtime::StableId::new(&__a11y_key), ::ui_lang_runtime::Role::GenericContainer).logical_id_maybe(::core::cfg!(test).then_some(__a11y_key)).into() };
#[cfg(test)]
drop(__ice_render_source);
__ice_rendered
}); } if ((!false) && (!self.loading)) { __children.push({
#[cfg(test)]
let __ice_render_source_location = ::ui_lang_runtime::testing::Location::new("components/shell.ice", 93, 1, "rendered view node");
#[cfg(test)]
let __ice_render_source = ::ui_lang_runtime::testing::push_render_source(__ice_render_source_location);
let __ice_rendered: __IceElement<'_, __DucktapeMessage> = { let __a11y_key = format!("{}/@container:93", __ice_use_scope); let __container_content: __IceElement<'_, __DucktapeMessage> = {
#[cfg(test)]
let __ice_render_source_location = ::ui_lang_runtime::testing::Location::new("components/shell.ice", 99, 1, "rendered view node");
#[cfg(test)]
let __ice_render_source = ::ui_lang_runtime::testing::push_render_source(__ice_render_source_location);
let __ice_rendered: __IceElement<'_, __DucktapeMessage> = ::iced::widget::space().width(1.0 as f32).height(1.0 as f32).into();
#[cfg(test)]
drop(__ice_render_source);
__ice_rendered
}; let __container = {  ::iced::widget::container(__container_content).width(7.0 as f32).height(7.0 as f32).style(move |__theme| { let mut __style = ::iced::widget::container::Style::default(); __style.background = ::std::option::Option::Some(::iced::Background::Color(__ice_palette.colors[29])); __style.border.radius = ::iced::border::Radius { top_left: (((7.0 / 2.0)) as f32).max(0.0).min(f32::MAX), top_right: (((7.0 / 2.0)) as f32).max(0.0).min(f32::MAX), bottom_right: (((7.0 / 2.0)) as f32).max(0.0).min(f32::MAX), bottom_left: (((7.0 / 2.0)) as f32).max(0.0).min(f32::MAX) }; __style }) }; ::ui_lang_runtime::accessible(__container, ::ui_lang_runtime::StableId::new(&__a11y_key), ::ui_lang_runtime::Role::GenericContainer).logical_id_maybe(::core::cfg!(test).then_some(__a11y_key)).into() };
#[cfg(test)]
drop(__ice_render_source);
__ice_rendered
}); } let __child_count = __children.len(); let __children = __children.into_iter().map(|__child| ::ui_lang_runtime::bounded_fill_element(__child, __child_count, false)).collect::<::std::vec::Vec<_>>(); let __layout = ::iced::widget::column(__children); let __content = ::iced::widget::container(__layout); let __layout_content: __IceElement<'_, __DucktapeMessage> = __content.into(); let __a11y_key = __ice_node_scope.as_str(); ::ui_lang_runtime::accessible(__layout_content, ::ui_lang_runtime::StableId::new(&__a11y_key), ::ui_lang_runtime::Role::GenericContainer).logical_id_maybe(::core::cfg!(test).then_some(__a11y_key)).into() } };
#[cfg(test)]
drop(__ice_render_source);
__ice_rendered
}; __component_content }
#[cfg(test)]
pub(super) fn __ice_component_use_101(&self, __ice_palette: __IcePalette, __ice_use_scope: ::std::string::String) -> __IceElement<'_, __DucktapeMessage> { let __component_content: __IceElement<'_, __DucktapeMessage> = {
#[cfg(test)]
let __ice_render_source_location = ::ui_lang_runtime::testing::Location::new("components/shell.ice", 166, 1, "rendered view node");
#[cfg(test)]
let __ice_render_source = ::ui_lang_runtime::testing::push_render_source(__ice_render_source_location);
let __ice_rendered: __IceElement<'_, __DucktapeMessage> = { let __ice_node_scope = format!("{}/root", __ice_use_scope); { let __a11y_key = __ice_node_scope.as_str(); let __container_content: __IceElement<'_, __DucktapeMessage> = {
#[cfg(test)]
let __ice_render_source_location = ::ui_lang_runtime::testing::Location::new("components/shell.ice", 180, 1, "rendered view node");
#[cfg(test)]
let __ice_render_source = ::ui_lang_runtime::testing::push_render_source(__ice_render_source_location);
let __ice_rendered: __IceElement<'_, __DucktapeMessage> = { let mut __children: ::std::vec::Vec<__IceElement<'_, __DucktapeMessage>> = ::std::vec::Vec::new(); __children.push({
#[cfg(test)]
let __ice_render_source_location = ::ui_lang_runtime::testing::Location::new("components/shell.ice", 181, 1, "rendered view node");
#[cfg(test)]
let __ice_render_source = ::ui_lang_runtime::testing::push_render_source(__ice_render_source_location);
let __ice_rendered: __IceElement<'_, __DucktapeMessage> = { let mut __children: ::std::vec::Vec<__IceElement<'_, __DucktapeMessage>> = ::std::vec::Vec::new(); __children.push({
#[cfg(test)]
let __ice_render_source_location = ::ui_lang_runtime::testing::Location::new("components/shell.ice", 186, 1, "rendered view node");
#[cfg(test)]
let __ice_render_source = ::ui_lang_runtime::testing::push_render_source(__ice_render_source_location);
let __ice_rendered: __IceElement<'_, __DucktapeMessage> = { let __ice_component_0537461747573446f74_scope_3106 = format!("{}/StatusDot@3106", __ice_use_scope); ::ui_lang_runtime::rev_memo(323u64, (self.__ice_rev[9], __ice_palette.name), ::ui_lang_runtime::grow_stack(|| self.__ice_component_use_100(__ice_palette, __ice_component_0537461747573446f74_scope_3106))).into() };
#[cfg(test)]
drop(__ice_render_source);
__ice_rendered
}); if ((!false) && self.loading) { __children.push({
#[cfg(test)]
let __ice_render_source_location = ::ui_lang_runtime::testing::Location::new("components/shell.ice", 199, 1, "rendered view node");
#[cfg(test)]
let __ice_render_source = ::ui_lang_runtime::testing::push_render_source(__ice_render_source_location);
let __ice_rendered: __IceElement<'_, __DucktapeMessage> = { let __a11y_key = format!("{}/@text:199", __ice_use_scope); let __text_value = ("Syncing…").to_string(); let __text = ::iced::widget::text(__text_value.clone()).size(((12.0) as f32).max(f32::EPSILON).min(f32::MAX)).wrapping(::iced::widget::text::Wrapping::None).font(::iced::Font { family: ::iced::font::Family::Name("Geist"), weight: ::iced::font::Weight::Semibold, stretch: ::iced::font::Stretch::Normal, style: ::iced::font::Style::Normal }).color(__ice_palette.colors[7]); let __text = ::ui_lang_runtime::selectable_text(__text); ::ui_lang_runtime::accessible(__text, ::ui_lang_runtime::StableId::new(&__a11y_key), ::ui_lang_runtime::Role::Label).logical_id_maybe(::core::cfg!(test).then_some(__a11y_key)).value(__text_value).into() };
#[cfg(test)]
drop(__ice_render_source);
__ice_rendered
}); } if ((!false) && (!self.loading)) { __children.push({
#[cfg(test)]
let __ice_render_source_location = ::ui_lang_runtime::testing::Location::new("components/shell.ice", 206, 1, "rendered view node");
#[cfg(test)]
let __ice_render_source = ::ui_lang_runtime::testing::push_render_source(__ice_render_source_location);
let __ice_rendered: __IceElement<'_, __DucktapeMessage> = { let __a11y_key = format!("{}/@text:206", __ice_use_scope); let __text_value = ("Synced").to_string(); let __text = ::iced::widget::text(__text_value.clone()).size(((12.0) as f32).max(f32::EPSILON).min(f32::MAX)).wrapping(::iced::widget::text::Wrapping::None).font(::iced::Font { family: ::iced::font::Family::Name("Geist"), weight: ::iced::font::Weight::Semibold, stretch: ::iced::font::Stretch::Normal, style: ::iced::font::Style::Normal }).color(__ice_palette.colors[7]); let __text = ::ui_lang_runtime::selectable_text(__text); ::ui_lang_runtime::accessible(__text, ::ui_lang_runtime::StableId::new(&__a11y_key), ::ui_lang_runtime::Role::Label).logical_id_maybe(::core::cfg!(test).then_some(__a11y_key)).value(__text_value).into() };
#[cfg(test)]
drop(__ice_render_source);
__ice_rendered
}); } __children.push({
#[cfg(test)]
let __ice_render_source_location = ::ui_lang_runtime::testing::Location::new("components/shell.ice", 212, 1, "rendered view node");
#[cfg(test)]
let __ice_render_source = ::ui_lang_runtime::testing::push_render_source(__ice_render_source_location);
let __ice_rendered: __IceElement<'_, __DucktapeMessage> = ::iced::widget::space().width(::iced::Fill).into();
#[cfg(test)]
drop(__ice_render_source);
__ice_rendered
}); if (!("validator").is_empty()) { __children.push({
#[cfg(test)]
let __ice_render_source_location = ::ui_lang_runtime::testing::Location::new("components/shell.ice", 222, 1, "rendered view node");
#[cfg(test)]
let __ice_render_source = ::ui_lang_runtime::testing::push_render_source(__ice_render_source_location);
let __ice_rendered: __IceElement<'_, __DucktapeMessage> = { let __a11y_key = format!("{}/@text:222", __ice_use_scope); let __text_value = ("validator").to_string(); let __text = ::iced::widget::text(__text_value.clone()).size(((10.5) as f32).max(f32::EPSILON).min(f32::MAX)).wrapping(::iced::widget::text::Wrapping::None).font(::iced::Font { family: ::iced::font::Family::Name("Geist Mono"), weight: ::iced::font::Weight::Medium, stretch: ::iced::font::Stretch::Normal, style: ::iced::font::Style::Normal }).color(__ice_palette.colors[71]); let __text = ::ui_lang_runtime::selectable_text(__text); ::ui_lang_runtime::accessible(__text, ::ui_lang_runtime::StableId::new(&__a11y_key), ::ui_lang_runtime::Role::Label).logical_id_maybe(::core::cfg!(test).then_some(__a11y_key)).value(__text_value).into() };
#[cfg(test)]
drop(__ice_render_source);
__ice_rendered
}); } if (("validator").is_empty() && true) { __children.push({
#[cfg(test)]
let __ice_render_source_location = ::ui_lang_runtime::testing::Location::new("components/shell.ice", 229, 1, "rendered view node");
#[cfg(test)]
let __ice_render_source = ::ui_lang_runtime::testing::push_render_source(__ice_render_source_location);
let __ice_rendered: __IceElement<'_, __DucktapeMessage> = { let __a11y_key = format!("{}/@text:229", __ice_use_scope); let __text_value = ("standing unknown").to_string(); let __text = ::iced::widget::text(__text_value.clone()).size(((10.5) as f32).max(f32::EPSILON).min(f32::MAX)).wrapping(::iced::widget::text::Wrapping::None).font(::iced::Font { family: ::iced::font::Family::Name("Geist Mono"), weight: ::iced::font::Weight::Medium, stretch: ::iced::font::Stretch::Normal, style: ::iced::font::Style::Normal }).color(__ice_palette.colors[71]); let __text = ::ui_lang_runtime::selectable_text(__text); ::ui_lang_runtime::accessible(__text, ::ui_lang_runtime::StableId::new(&__a11y_key), ::ui_lang_runtime::Role::Label).logical_id_maybe(::core::cfg!(test).then_some(__a11y_key)).value(__text_value).into() };
#[cfg(test)]
drop(__ice_render_source);
__ice_rendered
}); } let __child_count = __children.len(); let __children = __children.into_iter().map(|__child| ::ui_lang_runtime::bounded_fill_element(__child, __child_count, true)).collect::<::std::vec::Vec<_>>(); let __layout = ::iced::widget::row(__children).spacing(::ui_lang_runtime::bounded_spacing(7.0, __child_count)).width(::iced::Fill).align_y(::iced::alignment::Vertical::Center); let __content = ::iced::widget::container(__layout); let __layout_content: __IceElement<'_, __DucktapeMessage> = __content.into(); let __a11y_key = format!("{}/@layout:181", __ice_use_scope); ::ui_lang_runtime::accessible(__layout_content, ::ui_lang_runtime::StableId::new(&__a11y_key), ::ui_lang_runtime::Role::GenericContainer).logical_id_maybe(::core::cfg!(test).then_some(__a11y_key)).into() };
#[cfg(test)]
drop(__ice_render_source);
__ice_rendered
}); __children.push({
#[cfg(test)]
let __ice_render_source_location = ::ui_lang_runtime::testing::Location::new("components/shell.ice", 235, 1, "rendered view node");
#[cfg(test)]
let __ice_render_source = ::ui_lang_runtime::testing::push_render_source(__ice_render_source_location);
let __ice_rendered: __IceElement<'_, __DucktapeMessage> = { let __a11y_key = format!("{}/@text:235", __ice_use_scope); let __text_value = (crate::backend::height_label(84912)).to_string(); let __text = ::iced::widget::text(__text_value.clone()).size(((14.0) as f32).max(f32::EPSILON).min(f32::MAX)).wrapping(::iced::widget::text::Wrapping::None).font(::iced::Font { family: ::iced::font::Family::Name("Geist Mono"), weight: ::iced::font::Weight::Semibold, stretch: ::iced::font::Stretch::Normal, style: ::iced::font::Style::Normal }).color(__ice_palette.colors[7]); let __text = ::ui_lang_runtime::selectable_text(__text); ::ui_lang_runtime::accessible(__text, ::ui_lang_runtime::StableId::new(&__a11y_key), ::ui_lang_runtime::Role::Label).logical_id_maybe(::core::cfg!(test).then_some(__a11y_key)).value(__text_value).into() };
#[cfg(test)]
drop(__ice_render_source);
__ice_rendered
}); if (!(crate::backend::sync_label(::std::convert::AsRef::as_ref(&(self.node_phase)), self.node_sync_applied, self.node_sync_target)).is_empty()) { __children.push({
#[cfg(test)]
let __ice_render_source_location = ::ui_lang_runtime::testing::Location::new("components/shell.ice", 246, 1, "rendered view node");
#[cfg(test)]
let __ice_render_source = ::ui_lang_runtime::testing::push_render_source(__ice_render_source_location);
let __ice_rendered: __IceElement<'_, __DucktapeMessage> = { let __a11y_key = format!("{}/@text:246", __ice_use_scope); let __text_value = (crate::backend::sync_label(::std::convert::AsRef::as_ref(&(self.node_phase)), self.node_sync_applied, self.node_sync_target)).to_string(); let __text = ::iced::widget::text(__text_value.clone()).size(((11.0) as f32).max(f32::EPSILON).min(f32::MAX)).wrapping(::iced::widget::text::Wrapping::None).color(__ice_palette.colors[71]); let __text = ::ui_lang_runtime::selectable_text(__text); ::ui_lang_runtime::accessible(__text, ::ui_lang_runtime::StableId::new(&__a11y_key), ::ui_lang_runtime::Role::Label).logical_id_maybe(::core::cfg!(test).then_some(__a11y_key)).value(__text_value).into() };
#[cfg(test)]
drop(__ice_render_source);
__ice_rendered
}); } __children.push({
#[cfg(test)]
let __ice_render_source_location = ::ui_lang_runtime::testing::Location::new("components/shell.ice", 251, 1, "rendered view node");
#[cfg(test)]
let __ice_render_source = ::ui_lang_runtime::testing::push_render_source(__ice_render_source_location);
let __ice_rendered: __IceElement<'_, __DucktapeMessage> = { let mut __children: ::std::vec::Vec<__IceElement<'_, __DucktapeMessage>> = ::std::vec::Vec::new(); __children.push({
#[cfg(test)]
let __ice_render_source_location = ::ui_lang_runtime::testing::Location::new("components/shell.ice", 252, 1, "rendered view node");
#[cfg(test)]
let __ice_render_source = ::ui_lang_runtime::testing::push_render_source(__ice_render_source_location);
let __ice_rendered: __IceElement<'_, __DucktapeMessage> = { let mut __children: ::std::vec::Vec<__IceElement<'_, __DucktapeMessage>> = ::std::vec::Vec::new(); __children.push({
#[cfg(test)]
let __ice_render_source_location = ::ui_lang_runtime::testing::Location::new("components/shell.ice", 257, 1, "rendered view node");
#[cfg(test)]
let __ice_render_source = ::ui_lang_runtime::testing::push_render_source(__ice_render_source_location);
let __ice_rendered: __IceElement<'_, __DucktapeMessage> = { let __a11y_key = format!("{}/@text:257", __ice_use_scope); let __text_value = ("app-hash").to_string(); let __text = ::iced::widget::text(__text_value.clone()).size(((10.5) as f32).max(f32::EPSILON).min(f32::MAX)).wrapping(::iced::widget::text::Wrapping::None).font(::iced::Font { family: ::iced::font::Family::Name("Geist Mono"), weight: ::iced::font::Weight::Medium, stretch: ::iced::font::Stretch::Normal, style: ::iced::font::Style::Normal }).color(__ice_palette.colors[72]); let __text = ::ui_lang_runtime::selectable_text(__text); ::ui_lang_runtime::accessible(__text, ::ui_lang_runtime::StableId::new(&__a11y_key), ::ui_lang_runtime::Role::Label).logical_id_maybe(::core::cfg!(test).then_some(__a11y_key)).value(__text_value).into() };
#[cfg(test)]
drop(__ice_render_source);
__ice_rendered
}); __children.push({
#[cfg(test)]
let __ice_render_source_location = ::ui_lang_runtime::testing::Location::new("components/shell.ice", 263, 1, "rendered view node");
#[cfg(test)]
let __ice_render_source = ::ui_lang_runtime::testing::push_render_source(__ice_render_source_location);
let __ice_rendered: __IceElement<'_, __DucktapeMessage> = { let __a11y_key = format!("{}/@container:263", __ice_use_scope); let __container_content: __IceElement<'_, __DucktapeMessage> = {
#[cfg(test)]
let __ice_render_source_location = ::ui_lang_runtime::testing::Location::new("components/shell.ice", 264, 1, "rendered view node");
#[cfg(test)]
let __ice_render_source = ::ui_lang_runtime::testing::push_render_source(__ice_render_source_location);
let __ice_rendered: __IceElement<'_, __DucktapeMessage> = { let mut __children: ::std::vec::Vec<__IceElement<'_, __DucktapeMessage>> = ::std::vec::Vec::new(); __children.push({
#[cfg(test)]
let __ice_render_source_location = ::ui_lang_runtime::testing::Location::new("components/shell.ice", 265, 1, "rendered view node");
#[cfg(test)]
let __ice_render_source = ::ui_lang_runtime::testing::push_render_source(__ice_render_source_location);
let __ice_rendered: __IceElement<'_, __DucktapeMessage> = ::iced::widget::space().width(::iced::Fill).into();
#[cfg(test)]
drop(__ice_render_source);
__ice_rendered
}); __children.push({
#[cfg(test)]
let __ice_render_source_location = ::ui_lang_runtime::testing::Location::new("components/shell.ice", 266, 1, "rendered view node");
#[cfg(test)]
let __ice_render_source = ::ui_lang_runtime::testing::push_render_source(__ice_render_source_location);
let __ice_rendered: __IceElement<'_, __DucktapeMessage> = { let __a11y_key = format!("{}/@text:266", __ice_use_scope); let __text_value = ("").to_string(); let __text = ::iced::widget::text(__text_value.clone()).size(((10.5) as f32).max(f32::EPSILON).min(f32::MAX)).wrapping(::iced::widget::text::Wrapping::None).font(::iced::Font { family: ::iced::font::Family::Name("Geist Mono"), weight: ::iced::font::Weight::Medium, stretch: ::iced::font::Stretch::Normal, style: ::iced::font::Style::Normal }).color(__ice_palette.colors[13]); let __text = ::ui_lang_runtime::selectable_text(__text); ::ui_lang_runtime::accessible(__text, ::ui_lang_runtime::StableId::new(&__a11y_key), ::ui_lang_runtime::Role::Label).logical_id_maybe(::core::cfg!(test).then_some(__a11y_key)).value(__text_value).into() };
#[cfg(test)]
drop(__ice_render_source);
__ice_rendered
}); let __child_count = __children.len(); let __children = __children.into_iter().map(|__child| ::ui_lang_runtime::bounded_fill_element(__child, __child_count, true)).collect::<::std::vec::Vec<_>>(); let __layout = ::iced::widget::row(__children).width(::iced::Fill); let __content = ::iced::widget::container(__layout); let __layout_content: __IceElement<'_, __DucktapeMessage> = __content.into(); let __a11y_key = format!("{}/@layout:264", __ice_use_scope); ::ui_lang_runtime::accessible(__layout_content, ::ui_lang_runtime::StableId::new(&__a11y_key), ::ui_lang_runtime::Role::GenericContainer).logical_id_maybe(::core::cfg!(test).then_some(__a11y_key)).into() };
#[cfg(test)]
drop(__ice_render_source);
__ice_rendered
}; let __container = {  ::iced::widget::container(__container_content).width(::iced::Fill).clip(true) }; ::ui_lang_runtime::accessible(__container, ::ui_lang_runtime::StableId::new(&__a11y_key), ::ui_lang_runtime::Role::GenericContainer).logical_id_maybe(::core::cfg!(test).then_some(__a11y_key)).into() };
#[cfg(test)]
drop(__ice_render_source);
__ice_rendered
}); let __child_count = __children.len(); let __children = __children.into_iter().map(|__child| ::ui_lang_runtime::bounded_fill_element(__child, __child_count, true)).collect::<::std::vec::Vec<_>>(); let __layout = ::iced::widget::row(__children).spacing(::ui_lang_runtime::bounded_spacing(14.0, __child_count)).width(::iced::Fill).align_y(::iced::alignment::Vertical::Center); let __content = ::iced::widget::container(__layout); let __layout_content: __IceElement<'_, __DucktapeMessage> = __content.into(); let __a11y_key = format!("{}/@layout:252", __ice_use_scope); ::ui_lang_runtime::accessible(__layout_content, ::ui_lang_runtime::StableId::new(&__a11y_key), ::ui_lang_runtime::Role::GenericContainer).logical_id_maybe(::core::cfg!(test).then_some(__a11y_key)).into() };
#[cfg(test)]
drop(__ice_render_source);
__ice_rendered
}); __children.push({
#[cfg(test)]
let __ice_render_source_location = ::ui_lang_runtime::testing::Location::new("components/shell.ice", 272, 1, "rendered view node");
#[cfg(test)]
let __ice_render_source = ::ui_lang_runtime::testing::push_render_source(__ice_render_source_location);
let __ice_rendered: __IceElement<'_, __DucktapeMessage> = { let mut __children: ::std::vec::Vec<__IceElement<'_, __DucktapeMessage>> = ::std::vec::Vec::new(); __children.push({
#[cfg(test)]
let __ice_render_source_location = ::ui_lang_runtime::testing::Location::new("components/shell.ice", 277, 1, "rendered view node");
#[cfg(test)]
let __ice_render_source = ::ui_lang_runtime::testing::push_render_source(__ice_render_source_location);
let __ice_rendered: __IceElement<'_, __DucktapeMessage> = { let __a11y_key = format!("{}/@text:277", __ice_use_scope); let __text_value = ("finality").to_string(); let __text = ::iced::widget::text(__text_value.clone()).size(((10.5) as f32).max(f32::EPSILON).min(f32::MAX)).wrapping(::iced::widget::text::Wrapping::None).font(::iced::Font { family: ::iced::font::Family::Name("Geist Mono"), weight: ::iced::font::Weight::Medium, stretch: ::iced::font::Stretch::Normal, style: ::iced::font::Style::Normal }).color(__ice_palette.colors[72]); let __text = ::ui_lang_runtime::selectable_text(__text); ::ui_lang_runtime::accessible(__text, ::ui_lang_runtime::StableId::new(&__a11y_key), ::ui_lang_runtime::Role::Label).logical_id_maybe(::core::cfg!(test).then_some(__a11y_key)).value(__text_value).into() };
#[cfg(test)]
drop(__ice_render_source);
__ice_rendered
}); __children.push({
#[cfg(test)]
let __ice_render_source_location = ::ui_lang_runtime::testing::Location::new("components/shell.ice", 283, 1, "rendered view node");
#[cfg(test)]
let __ice_render_source = ::ui_lang_runtime::testing::push_render_source(__ice_render_source_location);
let __ice_rendered: __IceElement<'_, __DucktapeMessage> = ::iced::widget::space().width(::iced::Fill).into();
#[cfg(test)]
drop(__ice_render_source);
__ice_rendered
}); __children.push({
#[cfg(test)]
let __ice_render_source_location = ::ui_lang_runtime::testing::Location::new("components/shell.ice", 284, 1, "rendered view node");
#[cfg(test)]
let __ice_render_source = ::ui_lang_runtime::testing::push_render_source(__ice_render_source_location);
let __ice_rendered: __IceElement<'_, __DucktapeMessage> = { let mut __children: ::std::vec::Vec<__IceElement<'_, __DucktapeMessage>> = ::std::vec::Vec::new(); __children.push({
#[cfg(test)]
let __ice_render_source_location = ::ui_lang_runtime::testing::Location::new("components/shell.ice", 285, 1, "rendered view node");
#[cfg(test)]
let __ice_render_source = ::ui_lang_runtime::testing::push_render_source(__ice_render_source_location);
let __ice_rendered: __IceElement<'_, __DucktapeMessage> = { let __a11y_key = format!("{}/@text:285", __ice_use_scope); let __text_value = ("view").to_string(); let __text = ::iced::widget::text(__text_value.clone()).size(((10.5) as f32).max(f32::EPSILON).min(f32::MAX)).wrapping(::iced::widget::text::Wrapping::None).font(::iced::Font { family: ::iced::font::Family::Name("Geist Mono"), weight: ::iced::font::Weight::Medium, stretch: ::iced::font::Stretch::Normal, style: ::iced::font::Style::Normal }).color(__ice_palette.colors[13]); let __text = ::ui_lang_runtime::selectable_text(__text); ::ui_lang_runtime::accessible(__text, ::ui_lang_runtime::StableId::new(&__a11y_key), ::ui_lang_runtime::Role::Label).logical_id_maybe(::core::cfg!(test).then_some(__a11y_key)).value(__text_value).into() };
#[cfg(test)]
drop(__ice_render_source);
__ice_rendered
}); __children.push({
#[cfg(test)]
let __ice_render_source_location = ::ui_lang_runtime::testing::Location::new("components/shell.ice", 291, 1, "rendered view node");
#[cfg(test)]
let __ice_render_source = ::ui_lang_runtime::testing::push_render_source(__ice_render_source_location);
let __ice_rendered: __IceElement<'_, __DucktapeMessage> = { let __a11y_key = format!("{}/@text:291", __ice_use_scope); let __text_value = ("—").to_string(); let __text = ::iced::widget::text(__text_value.clone()).size(((10.5) as f32).max(f32::EPSILON).min(f32::MAX)).wrapping(::iced::widget::text::Wrapping::None).font(::iced::Font { family: ::iced::font::Family::Name("Geist Mono"), weight: ::iced::font::Weight::Medium, stretch: ::iced::font::Stretch::Normal, style: ::iced::font::Style::Normal }).color(__ice_palette.colors[13]); let __text = ::ui_lang_runtime::selectable_text(__text); ::ui_lang_runtime::accessible(__text, ::ui_lang_runtime::StableId::new(&__a11y_key), ::ui_lang_runtime::Role::Label).logical_id_maybe(::core::cfg!(test).then_some(__a11y_key)).value(__text_value).into() };
#[cfg(test)]
drop(__ice_render_source);
__ice_rendered
}); __children.push({
#[cfg(test)]
let __ice_render_source_location = ::ui_lang_runtime::testing::Location::new("components/shell.ice", 297, 1, "rendered view node");
#[cfg(test)]
let __ice_render_source = ::ui_lang_runtime::testing::push_render_source(__ice_render_source_location);
let __ice_rendered: __IceElement<'_, __DucktapeMessage> = { let __a11y_key = format!("{}/@text:297", __ice_use_scope); let __text_value = ("·").to_string(); let __text = ::iced::widget::text(__text_value.clone()).size(((10.5) as f32).max(f32::EPSILON).min(f32::MAX)).wrapping(::iced::widget::text::Wrapping::None).font(::iced::Font { family: ::iced::font::Family::Name("Geist Mono"), weight: ::iced::font::Weight::Medium, stretch: ::iced::font::Stretch::Normal, style: ::iced::font::Style::Normal }).color(__ice_palette.colors[72]); let __text = ::ui_lang_runtime::selectable_text(__text); ::ui_lang_runtime::accessible(__text, ::ui_lang_runtime::StableId::new(&__a11y_key), ::ui_lang_runtime::Role::Label).logical_id_maybe(::core::cfg!(test).then_some(__a11y_key)).value(__text_value).into() };
#[cfg(test)]
drop(__ice_render_source);
__ice_rendered
}); __children.push({
#[cfg(test)]
let __ice_render_source_location = ::ui_lang_runtime::testing::Location::new("components/shell.ice", 303, 1, "rendered view node");
#[cfg(test)]
let __ice_render_source = ::ui_lang_runtime::testing::push_render_source(__ice_render_source_location);
let __ice_rendered: __IceElement<'_, __DucktapeMessage> = { let __a11y_key = format!("{}/@text:303", __ice_use_scope); let __text_value = ("—").to_string(); let __text = ::iced::widget::text(__text_value.clone()).size(((10.5) as f32).max(f32::EPSILON).min(f32::MAX)).wrapping(::iced::widget::text::Wrapping::None).font(::iced::Font { family: ::iced::font::Family::Name("Geist Mono"), weight: ::iced::font::Weight::Medium, stretch: ::iced::font::Stretch::Normal, style: ::iced::font::Style::Normal }).color(__ice_palette.colors[13]); let __text = ::ui_lang_runtime::selectable_text(__text); ::ui_lang_runtime::accessible(__text, ::ui_lang_runtime::StableId::new(&__a11y_key), ::ui_lang_runtime::Role::Label).logical_id_maybe(::core::cfg!(test).then_some(__a11y_key)).value(__text_value).into() };
#[cfg(test)]
drop(__ice_render_source);
__ice_rendered
}); __children.push({
#[cfg(test)]
let __ice_render_source_location = ::ui_lang_runtime::testing::Location::new("components/shell.ice", 309, 1, "rendered view node");
#[cfg(test)]
let __ice_render_source = ::ui_lang_runtime::testing::push_render_source(__ice_render_source_location);
let __ice_rendered: __IceElement<'_, __DucktapeMessage> = { let __a11y_key = format!("{}/@text:309", __ice_use_scope); let __text_value = ("/").to_string(); let __text = ::iced::widget::text(__text_value.clone()).size(((10.5) as f32).max(f32::EPSILON).min(f32::MAX)).wrapping(::iced::widget::text::Wrapping::None).font(::iced::Font { family: ::iced::font::Family::Name("Geist Mono"), weight: ::iced::font::Weight::Medium, stretch: ::iced::font::Stretch::Normal, style: ::iced::font::Style::Normal }).color(__ice_palette.colors[72]); let __text = ::ui_lang_runtime::selectable_text(__text); ::ui_lang_runtime::accessible(__text, ::ui_lang_runtime::StableId::new(&__a11y_key), ::ui_lang_runtime::Role::Label).logical_id_maybe(::core::cfg!(test).then_some(__a11y_key)).value(__text_value).into() };
#[cfg(test)]
drop(__ice_render_source);
__ice_rendered
}); __children.push({
#[cfg(test)]
let __ice_render_source_location = ::ui_lang_runtime::testing::Location::new("components/shell.ice", 315, 1, "rendered view node");
#[cfg(test)]
let __ice_render_source = ::ui_lang_runtime::testing::push_render_source(__ice_render_source_location);
let __ice_rendered: __IceElement<'_, __DucktapeMessage> = { let __a11y_key = format!("{}/@text:315", __ice_use_scope); let __text_value = ("—").to_string(); let __text = ::iced::widget::text(__text_value.clone()).size(((10.5) as f32).max(f32::EPSILON).min(f32::MAX)).wrapping(::iced::widget::text::Wrapping::None).font(::iced::Font { family: ::iced::font::Family::Name("Geist Mono"), weight: ::iced::font::Weight::Medium, stretch: ::iced::font::Stretch::Normal, style: ::iced::font::Style::Normal }).color(__ice_palette.colors[13]); let __text = ::ui_lang_runtime::selectable_text(__text); ::ui_lang_runtime::accessible(__text, ::ui_lang_runtime::StableId::new(&__a11y_key), ::ui_lang_runtime::Role::Label).logical_id_maybe(::core::cfg!(test).then_some(__a11y_key)).value(__text_value).into() };
#[cfg(test)]
drop(__ice_render_source);
__ice_rendered
}); __children.push({
#[cfg(test)]
let __ice_render_source_location = ::ui_lang_runtime::testing::Location::new("components/shell.ice", 321, 1, "rendered view node");
#[cfg(test)]
let __ice_render_source = ::ui_lang_runtime::testing::push_render_source(__ice_render_source_location);
let __ice_rendered: __IceElement<'_, __DucktapeMessage> = { let __a11y_key = format!("{}/@text:321", __ice_use_scope); let __text_value = ("certs").to_string(); let __text = ::iced::widget::text(__text_value.clone()).size(((10.5) as f32).max(f32::EPSILON).min(f32::MAX)).wrapping(::iced::widget::text::Wrapping::None).font(::iced::Font { family: ::iced::font::Family::Name("Geist Mono"), weight: ::iced::font::Weight::Medium, stretch: ::iced::font::Stretch::Normal, style: ::iced::font::Style::Normal }).color(__ice_palette.colors[13]); let __text = ::ui_lang_runtime::selectable_text(__text); ::ui_lang_runtime::accessible(__text, ::ui_lang_runtime::StableId::new(&__a11y_key), ::ui_lang_runtime::Role::Label).logical_id_maybe(::core::cfg!(test).then_some(__a11y_key)).value(__text_value).into() };
#[cfg(test)]
drop(__ice_render_source);
__ice_rendered
}); let __child_count = __children.len(); let __children = __children.into_iter().map(|__child| ::ui_lang_runtime::bounded_fill_element(__child, __child_count, true)).collect::<::std::vec::Vec<_>>(); let __layout = ::iced::widget::row(__children).spacing(::ui_lang_runtime::bounded_spacing(4.0, __child_count)).align_y(::iced::alignment::Vertical::Center); let __content = ::iced::widget::container(__layout); let __layout_content: __IceElement<'_, __DucktapeMessage> = __content.into(); let __a11y_key = format!("{}/@layout:284", __ice_use_scope); ::ui_lang_runtime::accessible(__layout_content, ::ui_lang_runtime::StableId::new(&__a11y_key), ::ui_lang_runtime::Role::GenericContainer).logical_id_maybe(::core::cfg!(test).then_some(__a11y_key)).into() };
#[cfg(test)]
drop(__ice_render_source);
__ice_rendered
}); let __child_count = __children.len(); let __children = __children.into_iter().map(|__child| ::ui_lang_runtime::bounded_fill_element(__child, __child_count, true)).collect::<::std::vec::Vec<_>>(); let __layout = ::iced::widget::row(__children).spacing(::ui_lang_runtime::bounded_spacing(14.0, __child_count)).width(::iced::Fill).align_y(::iced::alignment::Vertical::Center); let __content = ::iced::widget::container(__layout); let __layout_content: __IceElement<'_, __DucktapeMessage> = __content.into(); let __a11y_key = format!("{}/@layout:272", __ice_use_scope); ::ui_lang_runtime::accessible(__layout_content, ::ui_lang_runtime::StableId::new(&__a11y_key), ::ui_lang_runtime::Role::GenericContainer).logical_id_maybe(::core::cfg!(test).then_some(__a11y_key)).into() };
#[cfg(test)]
drop(__ice_render_source);
__ice_rendered
}); __children.push({
#[cfg(test)]
let __ice_render_source_location = ::ui_lang_runtime::testing::Location::new("components/shell.ice", 327, 1, "rendered view node");
#[cfg(test)]
let __ice_render_source = ::ui_lang_runtime::testing::push_render_source(__ice_render_source_location);
let __ice_rendered: __IceElement<'_, __DucktapeMessage> = { let mut __children: ::std::vec::Vec<__IceElement<'_, __DucktapeMessage>> = ::std::vec::Vec::new(); __children.push({
#[cfg(test)]
let __ice_render_source_location = ::ui_lang_runtime::testing::Location::new("components/shell.ice", 332, 1, "rendered view node");
#[cfg(test)]
let __ice_render_source = ::ui_lang_runtime::testing::push_render_source(__ice_render_source_location);
let __ice_rendered: __IceElement<'_, __DucktapeMessage> = { let __a11y_key = format!("{}/@text:332", __ice_use_scope); let __text_value = ("last block").to_string(); let __text = ::iced::widget::text(__text_value.clone()).size(((10.5) as f32).max(f32::EPSILON).min(f32::MAX)).wrapping(::iced::widget::text::Wrapping::None).font(::iced::Font { family: ::iced::font::Family::Name("Geist Mono"), weight: ::iced::font::Weight::Medium, stretch: ::iced::font::Stretch::Normal, style: ::iced::font::Style::Normal }).color(__ice_palette.colors[72]); let __text = ::ui_lang_runtime::selectable_text(__text); ::ui_lang_runtime::accessible(__text, ::ui_lang_runtime::StableId::new(&__a11y_key), ::ui_lang_runtime::Role::Label).logical_id_maybe(::core::cfg!(test).then_some(__a11y_key)).value(__text_value).into() };
#[cfg(test)]
drop(__ice_render_source);
__ice_rendered
}); __children.push({
#[cfg(test)]
let __ice_render_source_location = ::ui_lang_runtime::testing::Location::new("components/shell.ice", 338, 1, "rendered view node");
#[cfg(test)]
let __ice_render_source = ::ui_lang_runtime::testing::push_render_source(__ice_render_source_location);
let __ice_rendered: __IceElement<'_, __DucktapeMessage> = ::iced::widget::space().width(::iced::Fill).into();
#[cfg(test)]
drop(__ice_render_source);
__ice_rendered
}); __children.push({
#[cfg(test)]
let __ice_render_source_location = ::ui_lang_runtime::testing::Location::new("components/shell.ice", 339, 1, "rendered view node");
#[cfg(test)]
let __ice_render_source = ::ui_lang_runtime::testing::push_render_source(__ice_render_source_location);
let __ice_rendered: __IceElement<'_, __DucktapeMessage> = { let __a11y_key = format!("{}/@text:339", __ice_use_scope); let __text_value = (crate::backend::relative_time(0, self.wall_now)).to_string(); let __text = ::iced::widget::text(__text_value.clone()).size(((10.5) as f32).max(f32::EPSILON).min(f32::MAX)).wrapping(::iced::widget::text::Wrapping::None).font(::iced::Font { family: ::iced::font::Family::Name("Geist Mono"), weight: ::iced::font::Weight::Medium, stretch: ::iced::font::Stretch::Normal, style: ::iced::font::Style::Normal }).color(__ice_palette.colors[13]); let __text = ::ui_lang_runtime::selectable_text(__text); ::ui_lang_runtime::accessible(__text, ::ui_lang_runtime::StableId::new(&__a11y_key), ::ui_lang_runtime::Role::Label).logical_id_maybe(::core::cfg!(test).then_some(__a11y_key)).value(__text_value).into() };
#[cfg(test)]
drop(__ice_render_source);
__ice_rendered
}); let __child_count = __children.len(); let __children = __children.into_iter().map(|__child| ::ui_lang_runtime::bounded_fill_element(__child, __child_count, true)).collect::<::std::vec::Vec<_>>(); let __layout = ::iced::widget::row(__children).spacing(::ui_lang_runtime::bounded_spacing(14.0, __child_count)).width(::iced::Fill).align_y(::iced::alignment::Vertical::Center); let __content = ::iced::widget::container(__layout); let __layout_content: __IceElement<'_, __DucktapeMessage> = __content.into(); let __a11y_key = format!("{}/@layout:327", __ice_use_scope); ::ui_lang_runtime::accessible(__layout_content, ::ui_lang_runtime::StableId::new(&__a11y_key), ::ui_lang_runtime::Role::GenericContainer).logical_id_maybe(::core::cfg!(test).then_some(__a11y_key)).into() };
#[cfg(test)]
drop(__ice_render_source);
__ice_rendered
}); let __child_count = __children.len(); let __children = __children.into_iter().map(|__child| ::ui_lang_runtime::bounded_fill_element(__child, __child_count, false)).collect::<::std::vec::Vec<_>>(); let __layout = ::iced::widget::column(__children).spacing(::ui_lang_runtime::bounded_spacing(6.0, __child_count)).width(::iced::Fill); let __content = ::iced::widget::container(__layout); let __layout_content: __IceElement<'_, __DucktapeMessage> = __content.into(); let __a11y_key = format!("{}/@layout:251", __ice_use_scope); ::ui_lang_runtime::accessible(__layout_content, ::ui_lang_runtime::StableId::new(&__a11y_key), ::ui_lang_runtime::Role::GenericContainer).logical_id_maybe(::core::cfg!(test).then_some(__a11y_key)).into() };
#[cfg(test)]
drop(__ice_render_source);
__ice_rendered
}); __children.push({
#[cfg(test)]
let __ice_render_source_location = ::ui_lang_runtime::testing::Location::new("components/shell.ice", 345, 1, "rendered view node");
#[cfg(test)]
let __ice_render_source = ::ui_lang_runtime::testing::push_render_source(__ice_render_source_location);
let __ice_rendered: __IceElement<'_, __DucktapeMessage> = { let mut __children: ::std::vec::Vec<__IceElement<'_, __DucktapeMessage>> = ::std::vec::Vec::new(); __children.push({
#[cfg(test)]
let __ice_render_source_location = ::ui_lang_runtime::testing::Location::new("components/shell.ice", 346, 1, "rendered view node");
#[cfg(test)]
let __ice_render_source = ::ui_lang_runtime::testing::push_render_source(__ice_render_source_location);
let __ice_rendered: __IceElement<'_, __DucktapeMessage> = { let __a11y_key = format!("{}/@container:346", __ice_use_scope); let __container_content: __IceElement<'_, __DucktapeMessage> = {
#[cfg(test)]
let __ice_render_source_location = ::ui_lang_runtime::testing::Location::new("components/shell.ice", 351, 1, "rendered view node");
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
let __ice_render_source_location = ::ui_lang_runtime::testing::Location::new("components/shell.ice", 352, 1, "rendered view node");
#[cfg(test)]
let __ice_render_source = ::ui_lang_runtime::testing::push_render_source(__ice_render_source_location);
let __ice_rendered: __IceElement<'_, __DucktapeMessage> = { let __a11y_key = format!("{}/@text:352", __ice_use_scope); let __text_value = ("This node verifies every record itself · it takes no one's word for it").to_string(); let __text = ::iced::widget::text(__text_value.clone()).size(((12.5) as f32).max(f32::EPSILON).min(f32::MAX)).width(::iced::Fill).color(__ice_palette.colors[71]); let __text = ::ui_lang_runtime::selectable_text(__text); ::ui_lang_runtime::accessible(__text, ::ui_lang_runtime::StableId::new(&__a11y_key), ::ui_lang_runtime::Role::Label).logical_id_maybe(::core::cfg!(test).then_some(__a11y_key)).value(__text_value).into() };
#[cfg(test)]
drop(__ice_render_source);
__ice_rendered
}); let __child_count = __children.len(); let __children = __children.into_iter().map(|__child| ::ui_lang_runtime::bounded_fill_element(__child, __child_count, false)).collect::<::std::vec::Vec<_>>(); let __layout = ::iced::widget::column(__children).spacing(::ui_lang_runtime::bounded_spacing(9.0, __child_count)).width(::iced::Fill); let __content = ::iced::widget::container(__layout); let __layout_content: __IceElement<'_, __DucktapeMessage> = __content.into(); let __a11y_key = format!("{}/@layout:345", __ice_use_scope); ::ui_lang_runtime::accessible(__layout_content, ::ui_lang_runtime::StableId::new(&__a11y_key), ::ui_lang_runtime::Role::GenericContainer).logical_id_maybe(::core::cfg!(test).then_some(__a11y_key)).into() };
#[cfg(test)]
drop(__ice_render_source);
__ice_rendered
}); let __child_count = __children.len(); let __children = __children.into_iter().map(|__child| ::ui_lang_runtime::bounded_fill_element(__child, __child_count, false)).collect::<::std::vec::Vec<_>>(); let __layout = ::iced::widget::column(__children).spacing(::ui_lang_runtime::bounded_spacing(11.0, __child_count)).width(::iced::Fill); let __content = ::iced::widget::container(__layout); let __layout_content: __IceElement<'_, __DucktapeMessage> = __content.into(); let __a11y_key = format!("{}/@layout:180", __ice_use_scope); ::ui_lang_runtime::accessible(__layout_content, ::ui_lang_runtime::StableId::new(&__a11y_key), ::ui_lang_runtime::Role::GenericContainer).logical_id_maybe(::core::cfg!(test).then_some(__a11y_key)).into() };
#[cfg(test)]
drop(__ice_render_source);
__ice_rendered
}; let __container = {  ::iced::widget::container(__container_content).id(::iced::widget::Id::from(__ice_node_scope.clone())).padding(::ui_lang_runtime::bounded_padding(13.0, 14.0, 13.0, 14.0)).width(284.0 as f32).style(move |__theme| { let mut __style = ::iced::widget::container::Style::default(); __style.background = ::std::option::Option::Some(::iced::Background::Color(__ice_palette.colors[3])); __style.border.color = __ice_palette.colors[39]; __style.border.width = 1.0 as f32; __style.border.radius = ::iced::border::Radius { top_left: ((13.0) as f32).max(0.0).min(f32::MAX), top_right: ((13.0) as f32).max(0.0).min(f32::MAX), bottom_right: ((13.0) as f32).max(0.0).min(f32::MAX), bottom_left: ((13.0) as f32).max(0.0).min(f32::MAX) }; __style.shadow.color = __ice_palette.colors[48]; __style.shadow.offset.y = 16.0 as f32; __style.shadow.blur_radius = 40.0 as f32; __style }) }; ::ui_lang_runtime::accessible(__container, ::ui_lang_runtime::StableId::new(&__a11y_key), ::ui_lang_runtime::Role::GenericContainer).logical_id_maybe(::core::cfg!(test).then_some(__a11y_key)).into() } };
#[cfg(test)]
drop(__ice_render_source);
__ice_rendered
}; __component_content }
#[cfg(test)]
pub(super) fn __ice_component_use_104(&self, __ice_palette: __IcePalette, __ice_use_scope: ::std::string::String) -> __IceElement<'_, __DucktapeMessage> { let __component_content: __IceElement<'_, __DucktapeMessage> = {
#[cfg(test)]
let __ice_render_source_location = ::ui_lang_runtime::testing::Location::new("components/shell.ice", 364, 1, "rendered view node");
#[cfg(test)]
let __ice_render_source = ::ui_lang_runtime::testing::push_render_source(__ice_render_source_location);
let __ice_rendered: __IceElement<'_, __DucktapeMessage> = { let __ice_node_scope = format!("{}/root", __ice_use_scope); { let mut __children: ::std::vec::Vec<__IceElement<'_, __DucktapeMessage>> = ::std::vec::Vec::new(); if ("info" == "danger") { __children.push({
#[cfg(test)]
let __ice_render_source_location = ::ui_lang_runtime::testing::Location::new("components/shell.ice", 367, 1, "rendered view node");
#[cfg(test)]
let __ice_render_source = ::ui_lang_runtime::testing::push_render_source(__ice_render_source_location);
let __ice_rendered: __IceElement<'_, __DucktapeMessage> = { let __a11y_key = format!("{}/@container:367", __ice_use_scope); let __container_content: __IceElement<'_, __DucktapeMessage> = {
#[cfg(test)]
let __ice_render_source_location = ::ui_lang_runtime::testing::Location::new("components/shell.ice", 377, 1, "rendered view node");
#[cfg(test)]
let __ice_render_source = ::ui_lang_runtime::testing::push_render_source(__ice_render_source_location);
let __ice_rendered: __IceElement<'_, __DucktapeMessage> = { let __a11y_key = format!("{}/@text:377", __ice_use_scope); let __text_value = (0).to_string(); let __text = ::iced::widget::text(__text_value.clone()).size(((9.0) as f32).max(f32::EPSILON).min(f32::MAX)).wrapping(::iced::widget::text::Wrapping::None).font(::iced::Font { family: ::iced::font::Family::Name("Geist Mono"), weight: ::iced::font::Weight::Semibold, stretch: ::iced::font::Stretch::Normal, style: ::iced::font::Style::Normal }).color(__ice_palette.colors[17]); let __text = ::ui_lang_runtime::selectable_text(__text); ::ui_lang_runtime::accessible(__text, ::ui_lang_runtime::StableId::new(&__a11y_key), ::ui_lang_runtime::Role::Label).logical_id_maybe(::core::cfg!(test).then_some(__a11y_key)).value(__text_value).into() };
#[cfg(test)]
drop(__ice_render_source);
__ice_rendered
}; let __container = {  ::iced::widget::container(__container_content).width(13.0 as f32).height(13.0 as f32).align_x(::iced::alignment::Horizontal::Center).align_y(::iced::alignment::Vertical::Center).style(move |__theme| { let mut __style = ::iced::widget::container::Style::default(); __style.background = ::std::option::Option::Some(::iced::Background::Color(__ice_palette.colors[83])); __style.border.color = __ice_palette.colors[3]; __style.border.width = 1.5 as f32; __style.border.radius = ::iced::border::Radius { top_left: ((7.0) as f32).max(0.0).min(f32::MAX), top_right: ((7.0) as f32).max(0.0).min(f32::MAX), bottom_right: ((7.0) as f32).max(0.0).min(f32::MAX), bottom_left: ((7.0) as f32).max(0.0).min(f32::MAX) }; __style }) }; ::ui_lang_runtime::accessible(__container, ::ui_lang_runtime::StableId::new(&__a11y_key), ::ui_lang_runtime::Role::GenericContainer).logical_id_maybe(::core::cfg!(test).then_some(__a11y_key)).into() };
#[cfg(test)]
drop(__ice_render_source);
__ice_rendered
}); } if ((!("info" == "danger")) && ("info" == "warning")) { __children.push({
#[cfg(test)]
let __ice_render_source_location = ::ui_lang_runtime::testing::Location::new("components/shell.ice", 384, 1, "rendered view node");
#[cfg(test)]
let __ice_render_source = ::ui_lang_runtime::testing::push_render_source(__ice_render_source_location);
let __ice_rendered: __IceElement<'_, __DucktapeMessage> = { let __a11y_key = format!("{}/@container:384", __ice_use_scope); let __container_content: __IceElement<'_, __DucktapeMessage> = {
#[cfg(test)]
let __ice_render_source_location = ::ui_lang_runtime::testing::Location::new("components/shell.ice", 394, 1, "rendered view node");
#[cfg(test)]
let __ice_render_source = ::ui_lang_runtime::testing::push_render_source(__ice_render_source_location);
let __ice_rendered: __IceElement<'_, __DucktapeMessage> = { let __a11y_key = format!("{}/@text:394", __ice_use_scope); let __text_value = (0).to_string(); let __text = ::iced::widget::text(__text_value.clone()).size(((9.0) as f32).max(f32::EPSILON).min(f32::MAX)).wrapping(::iced::widget::text::Wrapping::None).font(::iced::Font { family: ::iced::font::Family::Name("Geist Mono"), weight: ::iced::font::Weight::Semibold, stretch: ::iced::font::Stretch::Normal, style: ::iced::font::Style::Normal }).color(__ice_palette.colors[17]); let __text = ::ui_lang_runtime::selectable_text(__text); ::ui_lang_runtime::accessible(__text, ::ui_lang_runtime::StableId::new(&__a11y_key), ::ui_lang_runtime::Role::Label).logical_id_maybe(::core::cfg!(test).then_some(__a11y_key)).value(__text_value).into() };
#[cfg(test)]
drop(__ice_render_source);
__ice_rendered
}; let __container = {  ::iced::widget::container(__container_content).width(13.0 as f32).height(13.0 as f32).align_x(::iced::alignment::Horizontal::Center).align_y(::iced::alignment::Vertical::Center).style(move |__theme| { let mut __style = ::iced::widget::container::Style::default(); __style.background = ::std::option::Option::Some(::iced::Background::Color(__ice_palette.colors[34])); __style.border.color = __ice_palette.colors[3]; __style.border.width = 1.5 as f32; __style.border.radius = ::iced::border::Radius { top_left: ((7.0) as f32).max(0.0).min(f32::MAX), top_right: ((7.0) as f32).max(0.0).min(f32::MAX), bottom_right: ((7.0) as f32).max(0.0).min(f32::MAX), bottom_left: ((7.0) as f32).max(0.0).min(f32::MAX) }; __style }) }; ::ui_lang_runtime::accessible(__container, ::ui_lang_runtime::StableId::new(&__a11y_key), ::ui_lang_runtime::Role::GenericContainer).logical_id_maybe(::core::cfg!(test).then_some(__a11y_key)).into() };
#[cfg(test)]
drop(__ice_render_source);
__ice_rendered
}); } if (!(("info" == "danger") || ("info" == "warning"))) { __children.push({
#[cfg(test)]
let __ice_render_source_location = ::ui_lang_runtime::testing::Location::new("components/shell.ice", 401, 1, "rendered view node");
#[cfg(test)]
let __ice_render_source = ::ui_lang_runtime::testing::push_render_source(__ice_render_source_location);
let __ice_rendered: __IceElement<'_, __DucktapeMessage> = { let __a11y_key = format!("{}/@container:401", __ice_use_scope); let __container_content: __IceElement<'_, __DucktapeMessage> = {
#[cfg(test)]
let __ice_render_source_location = ::ui_lang_runtime::testing::Location::new("components/shell.ice", 411, 1, "rendered view node");
#[cfg(test)]
let __ice_render_source = ::ui_lang_runtime::testing::push_render_source(__ice_render_source_location);
let __ice_rendered: __IceElement<'_, __DucktapeMessage> = { let __a11y_key = format!("{}/@text:411", __ice_use_scope); let __text_value = (0).to_string(); let __text = ::iced::widget::text(__text_value.clone()).size(((9.0) as f32).max(f32::EPSILON).min(f32::MAX)).wrapping(::iced::widget::text::Wrapping::None).font(::iced::Font { family: ::iced::font::Family::Name("Geist Mono"), weight: ::iced::font::Weight::Semibold, stretch: ::iced::font::Stretch::Normal, style: ::iced::font::Style::Normal }).color(__ice_palette.colors[17]); let __text = ::ui_lang_runtime::selectable_text(__text); ::ui_lang_runtime::accessible(__text, ::ui_lang_runtime::StableId::new(&__a11y_key), ::ui_lang_runtime::Role::Label).logical_id_maybe(::core::cfg!(test).then_some(__a11y_key)).value(__text_value).into() };
#[cfg(test)]
drop(__ice_render_source);
__ice_rendered
}; let __container = {  ::iced::widget::container(__container_content).width(13.0 as f32).height(13.0 as f32).align_x(::iced::alignment::Horizontal::Center).align_y(::iced::alignment::Vertical::Center).style(move |__theme| { let mut __style = ::iced::widget::container::Style::default(); __style.background = ::std::option::Option::Some(::iced::Background::Color(__ice_palette.colors[79])); __style.border.color = __ice_palette.colors[3]; __style.border.width = 1.5 as f32; __style.border.radius = ::iced::border::Radius { top_left: ((7.0) as f32).max(0.0).min(f32::MAX), top_right: ((7.0) as f32).max(0.0).min(f32::MAX), bottom_right: ((7.0) as f32).max(0.0).min(f32::MAX), bottom_left: ((7.0) as f32).max(0.0).min(f32::MAX) }; __style }) }; ::ui_lang_runtime::accessible(__container, ::ui_lang_runtime::StableId::new(&__a11y_key), ::ui_lang_runtime::Role::GenericContainer).logical_id_maybe(::core::cfg!(test).then_some(__a11y_key)).into() };
#[cfg(test)]
drop(__ice_render_source);
__ice_rendered
}); } let __child_count = __children.len(); let __children = __children.into_iter().map(|__child| ::ui_lang_runtime::bounded_fill_element(__child, __child_count, false)).collect::<::std::vec::Vec<_>>(); let __layout = ::iced::widget::column(__children); let __content = ::iced::widget::container(__layout); let __layout_content: __IceElement<'_, __DucktapeMessage> = __content.into(); let __a11y_key = __ice_node_scope.as_str(); ::ui_lang_runtime::accessible(__layout_content, ::ui_lang_runtime::StableId::new(&__a11y_key), ::ui_lang_runtime::Role::GenericContainer).logical_id_maybe(::core::cfg!(test).then_some(__a11y_key)).into() } };
#[cfg(test)]
drop(__ice_render_source);
__ice_rendered
}; __component_content }
#[cfg(test)]
pub(super) fn __ice_component_use_105(&self, __ice_palette: __IcePalette, __ice_use_scope: ::std::string::String) -> __IceElement<'_, __DucktapeMessage> { let __component_content: __IceElement<'_, __DucktapeMessage> = {
#[cfg(test)]
let __ice_render_source_location = ::ui_lang_runtime::testing::Location::new("components/shell.ice", 364, 1, "rendered view node");
#[cfg(test)]
let __ice_render_source = ::ui_lang_runtime::testing::push_render_source(__ice_render_source_location);
let __ice_rendered: __IceElement<'_, __DucktapeMessage> = { let __ice_node_scope = format!("{}/root", __ice_use_scope); { let mut __children: ::std::vec::Vec<__IceElement<'_, __DucktapeMessage>> = ::std::vec::Vec::new(); if ("info" == "danger") { __children.push({
#[cfg(test)]
let __ice_render_source_location = ::ui_lang_runtime::testing::Location::new("components/shell.ice", 367, 1, "rendered view node");
#[cfg(test)]
let __ice_render_source = ::ui_lang_runtime::testing::push_render_source(__ice_render_source_location);
let __ice_rendered: __IceElement<'_, __DucktapeMessage> = { let __a11y_key = format!("{}/@container:367", __ice_use_scope); let __container_content: __IceElement<'_, __DucktapeMessage> = {
#[cfg(test)]
let __ice_render_source_location = ::ui_lang_runtime::testing::Location::new("components/shell.ice", 377, 1, "rendered view node");
#[cfg(test)]
let __ice_render_source = ::ui_lang_runtime::testing::push_render_source(__ice_render_source_location);
let __ice_rendered: __IceElement<'_, __DucktapeMessage> = { let __a11y_key = format!("{}/@text:377", __ice_use_scope); let __text_value = (0).to_string(); let __text = ::iced::widget::text(__text_value.clone()).size(((9.0) as f32).max(f32::EPSILON).min(f32::MAX)).wrapping(::iced::widget::text::Wrapping::None).font(::iced::Font { family: ::iced::font::Family::Name("Geist Mono"), weight: ::iced::font::Weight::Semibold, stretch: ::iced::font::Stretch::Normal, style: ::iced::font::Style::Normal }).color(__ice_palette.colors[17]); let __text = ::ui_lang_runtime::selectable_text(__text); ::ui_lang_runtime::accessible(__text, ::ui_lang_runtime::StableId::new(&__a11y_key), ::ui_lang_runtime::Role::Label).logical_id_maybe(::core::cfg!(test).then_some(__a11y_key)).value(__text_value).into() };
#[cfg(test)]
drop(__ice_render_source);
__ice_rendered
}; let __container = {  ::iced::widget::container(__container_content).width(18.0 as f32).height(13.0 as f32).align_x(::iced::alignment::Horizontal::Center).align_y(::iced::alignment::Vertical::Center).style(move |__theme| { let mut __style = ::iced::widget::container::Style::default(); __style.background = ::std::option::Option::Some(::iced::Background::Color(__ice_palette.colors[83])); __style.border.color = __ice_palette.colors[3]; __style.border.width = 1.5 as f32; __style.border.radius = ::iced::border::Radius { top_left: ((7.0) as f32).max(0.0).min(f32::MAX), top_right: ((7.0) as f32).max(0.0).min(f32::MAX), bottom_right: ((7.0) as f32).max(0.0).min(f32::MAX), bottom_left: ((7.0) as f32).max(0.0).min(f32::MAX) }; __style }) }; ::ui_lang_runtime::accessible(__container, ::ui_lang_runtime::StableId::new(&__a11y_key), ::ui_lang_runtime::Role::GenericContainer).logical_id_maybe(::core::cfg!(test).then_some(__a11y_key)).into() };
#[cfg(test)]
drop(__ice_render_source);
__ice_rendered
}); } if ((!("info" == "danger")) && ("info" == "warning")) { __children.push({
#[cfg(test)]
let __ice_render_source_location = ::ui_lang_runtime::testing::Location::new("components/shell.ice", 384, 1, "rendered view node");
#[cfg(test)]
let __ice_render_source = ::ui_lang_runtime::testing::push_render_source(__ice_render_source_location);
let __ice_rendered: __IceElement<'_, __DucktapeMessage> = { let __a11y_key = format!("{}/@container:384", __ice_use_scope); let __container_content: __IceElement<'_, __DucktapeMessage> = {
#[cfg(test)]
let __ice_render_source_location = ::ui_lang_runtime::testing::Location::new("components/shell.ice", 394, 1, "rendered view node");
#[cfg(test)]
let __ice_render_source = ::ui_lang_runtime::testing::push_render_source(__ice_render_source_location);
let __ice_rendered: __IceElement<'_, __DucktapeMessage> = { let __a11y_key = format!("{}/@text:394", __ice_use_scope); let __text_value = (0).to_string(); let __text = ::iced::widget::text(__text_value.clone()).size(((9.0) as f32).max(f32::EPSILON).min(f32::MAX)).wrapping(::iced::widget::text::Wrapping::None).font(::iced::Font { family: ::iced::font::Family::Name("Geist Mono"), weight: ::iced::font::Weight::Semibold, stretch: ::iced::font::Stretch::Normal, style: ::iced::font::Style::Normal }).color(__ice_palette.colors[17]); let __text = ::ui_lang_runtime::selectable_text(__text); ::ui_lang_runtime::accessible(__text, ::ui_lang_runtime::StableId::new(&__a11y_key), ::ui_lang_runtime::Role::Label).logical_id_maybe(::core::cfg!(test).then_some(__a11y_key)).value(__text_value).into() };
#[cfg(test)]
drop(__ice_render_source);
__ice_rendered
}; let __container = {  ::iced::widget::container(__container_content).width(18.0 as f32).height(13.0 as f32).align_x(::iced::alignment::Horizontal::Center).align_y(::iced::alignment::Vertical::Center).style(move |__theme| { let mut __style = ::iced::widget::container::Style::default(); __style.background = ::std::option::Option::Some(::iced::Background::Color(__ice_palette.colors[34])); __style.border.color = __ice_palette.colors[3]; __style.border.width = 1.5 as f32; __style.border.radius = ::iced::border::Radius { top_left: ((7.0) as f32).max(0.0).min(f32::MAX), top_right: ((7.0) as f32).max(0.0).min(f32::MAX), bottom_right: ((7.0) as f32).max(0.0).min(f32::MAX), bottom_left: ((7.0) as f32).max(0.0).min(f32::MAX) }; __style }) }; ::ui_lang_runtime::accessible(__container, ::ui_lang_runtime::StableId::new(&__a11y_key), ::ui_lang_runtime::Role::GenericContainer).logical_id_maybe(::core::cfg!(test).then_some(__a11y_key)).into() };
#[cfg(test)]
drop(__ice_render_source);
__ice_rendered
}); } if (!(("info" == "danger") || ("info" == "warning"))) { __children.push({
#[cfg(test)]
let __ice_render_source_location = ::ui_lang_runtime::testing::Location::new("components/shell.ice", 401, 1, "rendered view node");
#[cfg(test)]
let __ice_render_source = ::ui_lang_runtime::testing::push_render_source(__ice_render_source_location);
let __ice_rendered: __IceElement<'_, __DucktapeMessage> = { let __a11y_key = format!("{}/@container:401", __ice_use_scope); let __container_content: __IceElement<'_, __DucktapeMessage> = {
#[cfg(test)]
let __ice_render_source_location = ::ui_lang_runtime::testing::Location::new("components/shell.ice", 411, 1, "rendered view node");
#[cfg(test)]
let __ice_render_source = ::ui_lang_runtime::testing::push_render_source(__ice_render_source_location);
let __ice_rendered: __IceElement<'_, __DucktapeMessage> = { let __a11y_key = format!("{}/@text:411", __ice_use_scope); let __text_value = (0).to_string(); let __text = ::iced::widget::text(__text_value.clone()).size(((9.0) as f32).max(f32::EPSILON).min(f32::MAX)).wrapping(::iced::widget::text::Wrapping::None).font(::iced::Font { family: ::iced::font::Family::Name("Geist Mono"), weight: ::iced::font::Weight::Semibold, stretch: ::iced::font::Stretch::Normal, style: ::iced::font::Style::Normal }).color(__ice_palette.colors[17]); let __text = ::ui_lang_runtime::selectable_text(__text); ::ui_lang_runtime::accessible(__text, ::ui_lang_runtime::StableId::new(&__a11y_key), ::ui_lang_runtime::Role::Label).logical_id_maybe(::core::cfg!(test).then_some(__a11y_key)).value(__text_value).into() };
#[cfg(test)]
drop(__ice_render_source);
__ice_rendered
}; let __container = {  ::iced::widget::container(__container_content).width(18.0 as f32).height(13.0 as f32).align_x(::iced::alignment::Horizontal::Center).align_y(::iced::alignment::Vertical::Center).style(move |__theme| { let mut __style = ::iced::widget::container::Style::default(); __style.background = ::std::option::Option::Some(::iced::Background::Color(__ice_palette.colors[79])); __style.border.color = __ice_palette.colors[3]; __style.border.width = 1.5 as f32; __style.border.radius = ::iced::border::Radius { top_left: ((7.0) as f32).max(0.0).min(f32::MAX), top_right: ((7.0) as f32).max(0.0).min(f32::MAX), bottom_right: ((7.0) as f32).max(0.0).min(f32::MAX), bottom_left: ((7.0) as f32).max(0.0).min(f32::MAX) }; __style }) }; ::ui_lang_runtime::accessible(__container, ::ui_lang_runtime::StableId::new(&__a11y_key), ::ui_lang_runtime::Role::GenericContainer).logical_id_maybe(::core::cfg!(test).then_some(__a11y_key)).into() };
#[cfg(test)]
drop(__ice_render_source);
__ice_rendered
}); } let __child_count = __children.len(); let __children = __children.into_iter().map(|__child| ::ui_lang_runtime::bounded_fill_element(__child, __child_count, false)).collect::<::std::vec::Vec<_>>(); let __layout = ::iced::widget::column(__children); let __content = ::iced::widget::container(__layout); let __layout_content: __IceElement<'_, __DucktapeMessage> = __content.into(); let __a11y_key = __ice_node_scope.as_str(); ::ui_lang_runtime::accessible(__layout_content, ::ui_lang_runtime::StableId::new(&__a11y_key), ::ui_lang_runtime::Role::GenericContainer).logical_id_maybe(::core::cfg!(test).then_some(__a11y_key)).into() } };
#[cfg(test)]
drop(__ice_render_source);
__ice_rendered
}; __component_content }
#[cfg(test)]
pub(super) fn __ice_component_use_106(&self, __ice_palette: __IcePalette, __ice_use_scope: ::std::string::String, __ice_cb_1: impl Fn() -> __DucktapeMessage + Clone + 'static, __ice_cb_2: impl Fn() -> __DucktapeMessage + Clone + 'static) -> __IceElement<'_, __DucktapeMessage> { let __component_content: __IceElement<'_, __DucktapeMessage> = {
#[cfg(test)]
let __ice_render_source_location = ::ui_lang_runtime::testing::Location::new("components/shell.ice", 425, 1, "rendered view node");
#[cfg(test)]
let __ice_render_source = ::ui_lang_runtime::testing::push_render_source(__ice_render_source_location);
let __ice_rendered: __IceElement<'_, __DucktapeMessage> = { let __ice_node_scope = format!("{}/root", __ice_use_scope); { let mut __children: ::std::vec::Vec<__IceElement<'_, __DucktapeMessage>> = ::std::vec::Vec::new(); __children.push({
#[cfg(test)]
let __ice_render_source_location = ::ui_lang_runtime::testing::Location::new("components/shell.ice", 429, 1, "rendered view node");
#[cfg(test)]
let __ice_render_source = ::ui_lang_runtime::testing::push_render_source(__ice_render_source_location);
let __ice_rendered: __IceElement<'_, __DucktapeMessage> = { let __a11y_key = format!("{}/@container:429", __ice_use_scope); let __container_content: __IceElement<'_, __DucktapeMessage> = {
#[cfg(test)]
let __ice_render_source_location = ::ui_lang_runtime::testing::Location::new("components/shell.ice", 436, 1, "rendered view node");
#[cfg(test)]
let __ice_render_source = ::ui_lang_runtime::testing::push_render_source(__ice_render_source_location);
let __ice_rendered: __IceElement<'_, __DucktapeMessage> = { let mut __children: ::std::vec::Vec<__IceElement<'_, __DucktapeMessage>> = ::std::vec::Vec::new(); __children.push({
#[cfg(test)]
let __ice_render_source_location = ::ui_lang_runtime::testing::Location::new("components/shell.ice", 442, 1, "rendered view node");
#[cfg(test)]
let __ice_render_source = ::ui_lang_runtime::testing::push_render_source(__ice_render_source_location);
let __ice_rendered: __IceElement<'_, __DucktapeMessage> = { let __ice_component_04e6574776f726b43686970_scope_3362 = format!("{}/NetworkChip@3362", __ice_use_scope); ::ui_lang_runtime::rev_memo(378u64, (__ice_palette.name), ::ui_lang_runtime::grow_stack(|| self.__ice_component_use_97(__ice_palette, __ice_component_04e6574776f726b43686970_scope_3362, (__ice_cb_1).clone()))).into() };
#[cfg(test)]
drop(__ice_render_source);
__ice_rendered
}); __children.push({
#[cfg(test)]
let __ice_render_source_location = ::ui_lang_runtime::testing::Location::new("components/shell.ice", 445, 1, "rendered view node");
#[cfg(test)]
let __ice_render_source = ::ui_lang_runtime::testing::push_render_source(__ice_render_source_location);
let __ice_rendered: __IceElement<'_, __DucktapeMessage> = ::iced::widget::space().width(::iced::Fill).into();
#[cfg(test)]
drop(__ice_render_source);
__ice_rendered
}); __children.push({
#[cfg(test)]
let __ice_render_source_location = ::ui_lang_runtime::testing::Location::new("components/shell.ice", 446, 1, "rendered view node");
#[cfg(test)]
let __ice_render_source = ::ui_lang_runtime::testing::push_render_source(__ice_render_source_location);
let __ice_rendered: __IceElement<'_, __DucktapeMessage> = { let mut __children: ::std::vec::Vec<__IceElement<'_, __DucktapeMessage>> = ::std::vec::Vec::new(); __children.push({
#[cfg(test)]
let __ice_render_source_location = ::ui_lang_runtime::testing::Location::new("components/shell.ice", 455, 1, "rendered view node");
#[cfg(test)]
let __ice_render_source = ::ui_lang_runtime::testing::push_render_source(__ice_render_source_location);
let __ice_rendered: __IceElement<'_, __DucktapeMessage> = { let __tooltip_content: __IceElement<'_, __DucktapeMessage> = {
#[cfg(test)]
let __ice_render_source_location = ::ui_lang_runtime::testing::Location::new("components/shell.ice", 462, 1, "rendered view node");
#[cfg(test)]
let __ice_render_source = ::ui_lang_runtime::testing::push_render_source(__ice_render_source_location);
let __ice_rendered: __IceElement<'_, __DucktapeMessage> = { let __ice_component_053746174757350696c6c_scope_3382 = format!("{}/StatusPill@3382", __ice_use_scope); ::ui_lang_runtime::rev_memo(382u64, (self.__ice_rev[9], __ice_palette.name), ::ui_lang_runtime::grow_stack(|| self.__ice_component_use_99(__ice_palette, __ice_component_053746174757350696c6c_scope_3382))).into() };
#[cfg(test)]
drop(__ice_render_source);
__ice_rendered
}; let __tooltip_tip: __IceElement<'_, __DucktapeMessage> = {
#[cfg(test)]
let __ice_render_source_location = ::ui_lang_runtime::testing::Location::new("components/shell.ice", 463, 1, "rendered view node");
#[cfg(test)]
let __ice_render_source = ::ui_lang_runtime::testing::push_render_source(__ice_render_source_location);
let __ice_rendered: __IceElement<'_, __DucktapeMessage> = { let __a11y_key = format!("{}/@container:463", __ice_use_scope); let __container_content: __IceElement<'_, __DucktapeMessage> = {
#[cfg(test)]
let __ice_render_source_location = ::ui_lang_runtime::testing::Location::new("components/shell.ice", 464, 1, "rendered view node");
#[cfg(test)]
let __ice_render_source = ::ui_lang_runtime::testing::push_render_source(__ice_render_source_location);
let __ice_rendered: __IceElement<'_, __DucktapeMessage> = { let __ice_component_053746174757343617264_scope_3384 = format!("{}/StatusCard@3384", __ice_use_scope); ::ui_lang_runtime::rev_memo(384u64, (self.__ice_rev[3], self.__ice_rev[90], self.__ice_rev[92], self.__ice_rev[93], self.__ice_rev[9], __ice_palette.name), ::ui_lang_runtime::grow_stack(|| self.__ice_component_use_101(__ice_palette, __ice_component_053746174757343617264_scope_3384))).into() };
#[cfg(test)]
drop(__ice_render_source);
__ice_rendered
}; let __container = {  ::iced::widget::container(__container_content).padding(::ui_lang_runtime::bounded_padding(0.0, 13.0, 0.0, 0.0)) }; ::ui_lang_runtime::accessible(__container, ::ui_lang_runtime::StableId::new(&__a11y_key), ::ui_lang_runtime::Role::GenericContainer).logical_id_maybe(::core::cfg!(test).then_some(__a11y_key)).into() };
#[cfg(test)]
drop(__ice_render_source);
__ice_rendered
}; ::iced::widget::tooltip(__tooltip_content, __tooltip_tip, ::iced::widget::tooltip::Position::Bottom).gap(::ui_lang_runtime::bounded_table_metric(13.5, 1)).padding(::ui_lang_runtime::bounded_table_metric(0.0, 1)).delay(::std::time::Duration::from_millis(u64::try_from(90).unwrap_or(0))).snap_within_viewport(true).style(move |__theme| { let mut __style = ::iced::widget::container::transparent(__theme); __style }).into() };
#[cfg(test)]
drop(__ice_render_source);
__ice_rendered
}); __children.push({
#[cfg(test)]
let __ice_render_source_location = ::ui_lang_runtime::testing::Location::new("components/shell.ice", 478, 1, "rendered view node");
#[cfg(test)]
let __ice_render_source = ::ui_lang_runtime::testing::push_render_source(__ice_render_source_location);
let __ice_rendered: __IceElement<'_, __DucktapeMessage> = { let mut __children: ::std::vec::Vec<__IceElement<'_, __DucktapeMessage>> = ::std::vec::Vec::new(); __children.push({
#[cfg(test)]
let __ice_render_source_location = ::ui_lang_runtime::testing::Location::new("components/shell.ice", 479, 1, "rendered view node");
#[cfg(test)]
let __ice_render_source = ::ui_lang_runtime::testing::push_render_source(__ice_render_source_location);
let __ice_rendered: __IceElement<'_, __DucktapeMessage> = { let __a11y_key = format!("{}/@button:479", __ice_use_scope); let __a11y_id = ::ui_lang_runtime::StableId::new(&__a11y_key); let __disabled = false; let __activate = (__ice_cb_2)(); let __button_content: __IceElement<'_, __DucktapeMessage> = {
#[cfg(test)]
let __ice_render_source_location = ::ui_lang_runtime::testing::Location::new("components/shell.ice", 484, 1, "rendered view node");
#[cfg(test)]
let __ice_render_source = ::ui_lang_runtime::testing::push_render_source(__ice_render_source_location);
let __ice_rendered: __IceElement<'_, __DucktapeMessage> = { let mut __children: ::std::vec::Vec<__IceElement<'_, __DucktapeMessage>> = ::std::vec::Vec::new(); if (0 > 0) { __children.push({
#[cfg(test)]
let __ice_render_source_location = ::ui_lang_runtime::testing::Location::new("components/shell.ice", 486, 1, "rendered view node");
#[cfg(test)]
let __ice_render_source = ::ui_lang_runtime::testing::push_render_source(__ice_render_source_location);
let __ice_rendered: __IceElement<'_, __DucktapeMessage> = ::ui_lang_runtime::grow_stack(|| self.__ice_component_use_102(__ice_palette, format!("{}/Icon@3406", __ice_use_scope)));
#[cfg(test)]
drop(__ice_render_source);
__ice_rendered
}); } if (0 <= 0) { __children.push({
#[cfg(test)]
let __ice_render_source_location = ::ui_lang_runtime::testing::Location::new("components/shell.ice", 492, 1, "rendered view node");
#[cfg(test)]
let __ice_render_source = ::ui_lang_runtime::testing::push_render_source(__ice_render_source_location);
let __ice_rendered: __IceElement<'_, __DucktapeMessage> = ::ui_lang_runtime::grow_stack(|| self.__ice_component_use_103(__ice_palette, format!("{}/Icon@3412", __ice_use_scope)));
#[cfg(test)]
drop(__ice_render_source);
__ice_rendered
}); } let __child_count = __children.len(); let __children = __children.into_iter().map(|__child| ::ui_lang_runtime::bounded_fill_element(__child, __child_count, false)).collect::<::std::vec::Vec<_>>(); let __layout = ::iced::widget::column(__children).align_x(::iced::alignment::Horizontal::Center); let __content = ::iced::widget::container(__layout); let __layout_content: __IceElement<'_, __DucktapeMessage> = __content.into(); let __a11y_key = format!("{}/@layout:484", __ice_use_scope); ::ui_lang_runtime::accessible(__layout_content, ::ui_lang_runtime::StableId::new(&__a11y_key), ::ui_lang_runtime::Role::GenericContainer).logical_id_maybe(::core::cfg!(test).then_some(__a11y_key)).into() };
#[cfg(test)]
drop(__ice_render_source);
__ice_rendered
}; let __button = ::iced::widget::button(__button_content).padding(5.0 as f32).on_press_maybe(if __disabled { None } else { Some(__activate.clone()) }).style(move |__theme, __status| { let mut __style = ::iced::widget::button::primary(__theme, __status); let __background: Option<::iced::Color> = match __status { ::iced::widget::button::Status::Hovered => Some(__ice_palette.colors[14]), ::iced::widget::button::Status::Pressed => Some(__ice_palette.colors[39]), ::iced::widget::button::Status::Disabled => Some(::iced::Color::from_rgba8(0, 0, 0, 0.000000)), _ => Some(::iced::Color::from_rgba8(0, 0, 0, 0.000000)) }; if let Some(__background) = __background { __style.background = Some(::iced::Background::Color(__background)); } __style.text_color = __ice_palette.colors[4]; __style.border.radius = 7.0.into(); __style.background = ::std::option::Option::Some(::iced::Background::Color(::iced::Color::from_rgba8(0, 0, 0, 0.000000))); __style.text_color = __ice_palette.colors[5]; __style.border.color = ::iced::Color::from_rgba8(0, 0, 0, 0.000000); __style.border.width = 1.0 as f32; __style.border.radius = ::iced::border::Radius { top_left: ((7.0) as f32).max(0.0).min(f32::MAX), top_right: ((7.0) as f32).max(0.0).min(f32::MAX), bottom_right: ((7.0) as f32).max(0.0).min(f32::MAX), bottom_left: ((7.0) as f32).max(0.0).min(f32::MAX) }; match __status { ::iced::widget::button::Status::Hovered => { __style.background = ::std::option::Option::Some(::iced::Background::Color(__ice_palette.colors[3])); __style.text_color = __ice_palette.colors[4]; __style.border.color = ::iced::Color::from_rgba8(0, 0, 0, 0.000000); } ::iced::widget::button::Status::Pressed => { __style.background = ::std::option::Option::Some(::iced::Background::Color(__ice_palette.colors[56])); } _ => {} } if matches!(__status, ::iced::widget::button::Status::Disabled) { if let Some(::iced::Background::Color(mut __color)) = __style.background { __color.a *= 0.5; __style.background = Some(::iced::Background::Color(__color)); } __style.text_color.a *= 0.5; } __style }); ::ui_lang_runtime::accessible(__button, __a11y_id, ::ui_lang_runtime::Role::Button).logical_id_maybe(::core::cfg!(test).then_some(&*__a11y_key)).focus_id(::iced::widget::Id::from(__a11y_key)).label("Alerts".to_owned()).disabled(__disabled).on_activate_maybe(if __disabled { None } else { Some(__activate) }).focus_ring(__ice_palette.colors[42], 7.0).into() };
#[cfg(test)]
drop(__ice_render_source);
__ice_rendered
}); if ((0 > 0) && (0 < 10)) { __children.push({
#[cfg(test)]
let __ice_render_source_location = ::ui_lang_runtime::testing::Location::new("components/shell.ice", 504, 1, "rendered view node");
#[cfg(test)]
let __ice_render_source = ::ui_lang_runtime::testing::push_render_source(__ice_render_source_location);
let __ice_rendered: __IceElement<'_, __DucktapeMessage> = { let __pin_content: __IceElement<'_, __DucktapeMessage> = {
#[cfg(test)]
let __ice_render_source_location = ::ui_lang_runtime::testing::Location::new("components/shell.ice", 505, 1, "rendered view node");
#[cfg(test)]
let __ice_render_source = ::ui_lang_runtime::testing::push_render_source(__ice_render_source_location);
let __ice_rendered: __IceElement<'_, __DucktapeMessage> = { let __ice_component_042656c6c4261646765_scope_3425 = format!("{}/BellBadge@3425", __ice_use_scope); ::ui_lang_runtime::rev_memo(394u64, (__ice_palette.name), ::ui_lang_runtime::grow_stack(|| self.__ice_component_use_104(__ice_palette, __ice_component_042656c6c4261646765_scope_3425))).into() };
#[cfg(test)]
drop(__ice_render_source);
__ice_rendered
}; ::iced::widget::pin(__pin_content).x(12.0 as f32).y(1.0 as f32).into() };
#[cfg(test)]
drop(__ice_render_source);
__ice_rendered
}); } if (0 > 9) { __children.push({
#[cfg(test)]
let __ice_render_source_location = ::ui_lang_runtime::testing::Location::new("components/shell.ice", 511, 1, "rendered view node");
#[cfg(test)]
let __ice_render_source = ::ui_lang_runtime::testing::push_render_source(__ice_render_source_location);
let __ice_rendered: __IceElement<'_, __DucktapeMessage> = { let __pin_content: __IceElement<'_, __DucktapeMessage> = {
#[cfg(test)]
let __ice_render_source_location = ::ui_lang_runtime::testing::Location::new("components/shell.ice", 512, 1, "rendered view node");
#[cfg(test)]
let __ice_render_source = ::ui_lang_runtime::testing::push_render_source(__ice_render_source_location);
let __ice_rendered: __IceElement<'_, __DucktapeMessage> = { let __ice_component_042656c6c4261646765_scope_3432 = format!("{}/BellBadge@3432", __ice_use_scope); ::ui_lang_runtime::rev_memo(397u64, (__ice_palette.name), ::ui_lang_runtime::grow_stack(|| self.__ice_component_use_105(__ice_palette, __ice_component_042656c6c4261646765_scope_3432))).into() };
#[cfg(test)]
drop(__ice_render_source);
__ice_rendered
}; ::iced::widget::pin(__pin_content).x(7.0 as f32).y(1.0 as f32).into() };
#[cfg(test)]
drop(__ice_render_source);
__ice_rendered
}); } let __layout = ::ui_lang_runtime::zstack(__children).width(26.0 as f32).height(24.0 as f32); let __content = ::iced::widget::container(__layout); let __layout_content: __IceElement<'_, __DucktapeMessage> = __content.into(); let __a11y_key = format!("{}/@layout:478", __ice_use_scope); ::ui_lang_runtime::accessible(__layout_content, ::ui_lang_runtime::StableId::new(&__a11y_key), ::ui_lang_runtime::Role::GenericContainer).logical_id_maybe(::core::cfg!(test).then_some(__a11y_key)).into() };
#[cfg(test)]
drop(__ice_render_source);
__ice_rendered
}); let __child_count = __children.len(); let __children = __children.into_iter().map(|__child| ::ui_lang_runtime::bounded_fill_element(__child, __child_count, true)).collect::<::std::vec::Vec<_>>(); let __layout = ::iced::widget::row(__children).spacing(::ui_lang_runtime::bounded_spacing(6.0, __child_count)).align_y(::iced::alignment::Vertical::Center); let __content = ::iced::widget::container(__layout); let __layout_content: __IceElement<'_, __DucktapeMessage> = __content.into(); let __a11y_key = format!("{}/@layout:446", __ice_use_scope); ::ui_lang_runtime::accessible(__layout_content, ::ui_lang_runtime::StableId::new(&__a11y_key), ::ui_lang_runtime::Role::GenericContainer).logical_id_maybe(::core::cfg!(test).then_some(__a11y_key)).into() };
#[cfg(test)]
drop(__ice_render_source);
__ice_rendered
}); let __child_count = __children.len(); let __children = __children.into_iter().map(|__child| ::ui_lang_runtime::bounded_fill_element(__child, __child_count, true)).collect::<::std::vec::Vec<_>>(); let __layout = ::iced::widget::row(__children).spacing(::ui_lang_runtime::bounded_spacing(13.0, __child_count)).width(::iced::Fill).height(::iced::Fill).align_y(::iced::alignment::Vertical::Center); let __content = ::iced::widget::container(__layout); let __layout_content: __IceElement<'_, __DucktapeMessage> = __content.into(); let __a11y_key = format!("{}/@layout:436", __ice_use_scope); ::ui_lang_runtime::accessible(__layout_content, ::ui_lang_runtime::StableId::new(&__a11y_key), ::ui_lang_runtime::Role::GenericContainer).logical_id_maybe(::core::cfg!(test).then_some(__a11y_key)).into() };
#[cfg(test)]
drop(__ice_render_source);
__ice_rendered
}; let __container = {  ::iced::widget::container(__container_content).padding(::ui_lang_runtime::bounded_padding(0.0, 13.0, 0.0, (13.0 + crate::backend::titlebar_inset()))).width(::iced::Fill).height(39.0 as f32).style(move |__theme| { let mut __style = ::iced::widget::container::Style::default(); __style.background = ::std::option::Option::Some(::iced::Background::Color(__ice_palette.colors[55])); __style }) }; ::ui_lang_runtime::accessible(__container, ::ui_lang_runtime::StableId::new(&__a11y_key), ::ui_lang_runtime::Role::GenericContainer).logical_id_maybe(::core::cfg!(test).then_some(__a11y_key)).into() };
#[cfg(test)]
drop(__ice_render_source);
__ice_rendered
}); __children.push({
#[cfg(test)]
let __ice_render_source_location = ::ui_lang_runtime::testing::Location::new("components/shell.ice", 517, 1, "rendered view node");
#[cfg(test)]
let __ice_render_source = ::ui_lang_runtime::testing::push_render_source(__ice_render_source_location);
let __ice_rendered: __IceElement<'_, __DucktapeMessage> = { let __a11y_key = format!("{}/@container:517", __ice_use_scope); let __container_content: __IceElement<'_, __DucktapeMessage> = {
#[cfg(test)]
let __ice_render_source_location = ::ui_lang_runtime::testing::Location::new("components/shell.ice", 522, 1, "rendered view node");
#[cfg(test)]
let __ice_render_source = ::ui_lang_runtime::testing::push_render_source(__ice_render_source_location);
let __ice_rendered: __IceElement<'_, __DucktapeMessage> = ::iced::widget::space().width(1.0 as f32).height(1.0 as f32).into();
#[cfg(test)]
drop(__ice_render_source);
__ice_rendered
}; let __container = {  ::iced::widget::container(__container_content).width(::iced::Fill).height(1.0 as f32).style(move |__theme| { let mut __style = ::iced::widget::container::Style::default(); __style.background = ::std::option::Option::Some(::iced::Background::Color(__ice_palette.colors[39])); __style }) }; ::ui_lang_runtime::accessible(__container, ::ui_lang_runtime::StableId::new(&__a11y_key), ::ui_lang_runtime::Role::GenericContainer).logical_id_maybe(::core::cfg!(test).then_some(__a11y_key)).into() };
#[cfg(test)]
drop(__ice_render_source);
__ice_rendered
}); let __child_count = __children.len(); let __children = __children.into_iter().map(|__child| ::ui_lang_runtime::bounded_fill_element(__child, __child_count, false)).collect::<::std::vec::Vec<_>>(); let __layout = ::iced::widget::column(__children).width(::iced::Fill); let __content = ::iced::widget::container(__layout); let __layout_content: __IceElement<'_, __DucktapeMessage> = __content.into(); let __a11y_key = __ice_node_scope.as_str(); ::ui_lang_runtime::accessible(__layout_content, ::ui_lang_runtime::StableId::new(&__a11y_key), ::ui_lang_runtime::Role::GenericContainer).logical_id_maybe(::core::cfg!(test).then_some(__a11y_key)).into() } };
#[cfg(test)]
drop(__ice_render_source);
__ice_rendered
}; __component_content }
#[cfg(test)]
pub(super) fn __ice_component_use_107(&self, __ice_palette: __IcePalette, __ice_use_scope: ::std::string::String) -> __IceElement<'_, __DucktapeMessage> { let __component_content: __IceElement<'_, __DucktapeMessage> = {
#[cfg(test)]
let __ice_render_source_location = ::ui_lang_runtime::testing::Location::new("components/shell.ice", 20, 1, "rendered view node");
#[cfg(test)]
let __ice_render_source = ::ui_lang_runtime::testing::push_render_source(__ice_render_source_location);
let __ice_rendered: __IceElement<'_, __DucktapeMessage> = { let __ice_node_scope = format!("{}/root", __ice_use_scope); { let __a11y_key = __ice_node_scope.as_str(); let __container_content: __IceElement<'_, __DucktapeMessage> = {
#[cfg(test)]
let __ice_render_source_location = ::ui_lang_runtime::testing::Location::new("components/shell.ice", 28, 1, "rendered view node");
#[cfg(test)]
let __ice_render_source = ::ui_lang_runtime::testing::push_render_source(__ice_render_source_location);
let __ice_rendered: __IceElement<'_, __DucktapeMessage> = { let __a11y_key = format!("{}/@text:28", __ice_use_scope); let __text_value = ("D").to_string(); let __text = ::iced::widget::text(__text_value.clone()).size(((14.0) as f32).max(f32::EPSILON).min(f32::MAX)).wrapping(::iced::widget::text::Wrapping::None).font(::iced::Font { family: ::iced::font::Family::Name("Geist Mono"), weight: ::iced::font::Weight::Semibold, stretch: ::iced::font::Stretch::Normal, style: ::iced::font::Style::Normal }).color(__ice_palette.colors[38]); let __text = ::ui_lang_runtime::selectable_text(__text); ::ui_lang_runtime::accessible(__text, ::ui_lang_runtime::StableId::new(&__a11y_key), ::ui_lang_runtime::Role::Label).logical_id_maybe(::core::cfg!(test).then_some(__a11y_key)).value(__text_value).into() };
#[cfg(test)]
drop(__ice_render_source);
__ice_rendered
}; let __container = {  ::iced::widget::container(__container_content).id(::iced::widget::Id::from(__ice_node_scope.clone())).width(30.0 as f32).height(30.0 as f32).align_x(::iced::alignment::Horizontal::Center).align_y(::iced::alignment::Vertical::Center).style(move |__theme| { let mut __style = ::iced::widget::container::Style::default(); __style.background = ::std::option::Option::Some(::iced::Background::Color(__ice_palette.colors[7])); __style.border.radius = ::iced::border::Radius { top_left: ((9.0) as f32).max(0.0).min(f32::MAX), top_right: ((9.0) as f32).max(0.0).min(f32::MAX), bottom_right: ((9.0) as f32).max(0.0).min(f32::MAX), bottom_left: ((9.0) as f32).max(0.0).min(f32::MAX) }; __style }) }; ::ui_lang_runtime::accessible(__container, ::ui_lang_runtime::StableId::new(&__a11y_key), ::ui_lang_runtime::Role::GenericContainer).logical_id_maybe(::core::cfg!(test).then_some(__a11y_key)).into() } };
#[cfg(test)]
drop(__ice_render_source);
__ice_rendered
}; __component_content }
#[cfg(test)]
pub(super) fn __ice_component_use_111(&self, __ice_palette: __IcePalette, __ice_use_scope: ::std::string::String, __ice_cb_0: impl Fn(ShellTab) -> __DucktapeMessage + Clone + 'static, __ice_arg_0: crate::backend::NavItem) -> __IceElement<'_, __DucktapeMessage> { let __component_content: __IceElement<'_, __DucktapeMessage> = {
#[cfg(test)]
let __ice_render_source_location = ::ui_lang_runtime::testing::Location::new("components/shell.ice", 572, 1, "rendered view node");
#[cfg(test)]
let __ice_render_source = ::ui_lang_runtime::testing::push_render_source(__ice_render_source_location);
let __ice_rendered: __IceElement<'_, __DucktapeMessage> = { let __ice_node_scope = format!("{}/root", __ice_use_scope); { let mut __children: ::std::vec::Vec<__IceElement<'_, __DucktapeMessage>> = ::std::vec::Vec::new(); if __ice_arg_0.active { __children.push({
#[cfg(test)]
let __ice_render_source_location = ::ui_lang_runtime::testing::Location::new("components/shell.ice", 574, 1, "rendered view node");
#[cfg(test)]
let __ice_render_source = ::ui_lang_runtime::testing::push_render_source(__ice_render_source_location);
let __ice_rendered: __IceElement<'_, __DucktapeMessage> = { let __a11y_key = format!("{}/@button:574", __ice_use_scope); let __a11y_id = ::ui_lang_runtime::StableId::new(&__a11y_key); let __disabled = false; let __activate = (__ice_cb_0)(__ice_arg_0.id.clone()); let __button_content: __IceElement<'_, __DucktapeMessage> = { let __button_inner: __IceElement<'_, __DucktapeMessage> = {
#[cfg(test)]
let __ice_render_source_location = ::ui_lang_runtime::testing::Location::new("components/shell.ice", 581, 1, "rendered view node");
#[cfg(test)]
let __ice_render_source = ::ui_lang_runtime::testing::push_render_source(__ice_render_source_location);
let __ice_rendered: __IceElement<'_, __DucktapeMessage> = { let mut __children: ::std::vec::Vec<__IceElement<'_, __DucktapeMessage>> = ::std::vec::Vec::new(); __children.push({
#[cfg(test)]
let __ice_render_source_location = ::ui_lang_runtime::testing::Location::new("components/shell.ice", 587, 1, "rendered view node");
#[cfg(test)]
let __ice_render_source = ::ui_lang_runtime::testing::push_render_source(__ice_render_source_location);
let __ice_rendered: __IceElement<'_, __DucktapeMessage> = ::ui_lang_runtime::grow_stack(|| self.__ice_component_use_108(__ice_palette, format!("{}/Icon@3507", __ice_use_scope), __ice_arg_0.icon.to_owned()));
#[cfg(test)]
drop(__ice_render_source);
__ice_rendered
}); __children.push({
#[cfg(test)]
let __ice_render_source_location = ::ui_lang_runtime::testing::Location::new("components/shell.ice", 592, 1, "rendered view node");
#[cfg(test)]
let __ice_render_source = ::ui_lang_runtime::testing::push_render_source(__ice_render_source_location);
let __ice_rendered: __IceElement<'_, __DucktapeMessage> = { let __a11y_key = format!("{}/@text:592", __ice_use_scope); let __text_value = (__ice_arg_0.title).to_string(); let __text = ::iced::widget::text(__text_value.clone()).size(((9.5) as f32).max(f32::EPSILON).min(f32::MAX)).wrapping(::iced::widget::text::Wrapping::None).font(::iced::Font { family: ::iced::font::Family::Name("Geist"), weight: ::iced::font::Weight::Semibold, stretch: ::iced::font::Stretch::Normal, style: ::iced::font::Style::Normal }).color(__ice_palette.colors[69]); let __text = ::ui_lang_runtime::selectable_text(__text); ::ui_lang_runtime::accessible(__text, ::ui_lang_runtime::StableId::new(&__a11y_key), ::ui_lang_runtime::Role::Label).logical_id_maybe(::core::cfg!(test).then_some(__a11y_key)).value(__text_value).into() };
#[cfg(test)]
drop(__ice_render_source);
__ice_rendered
}); let __child_count = __children.len(); let __children = __children.into_iter().map(|__child| ::ui_lang_runtime::bounded_fill_element(__child, __child_count, false)).collect::<::std::vec::Vec<_>>(); let __layout = ::iced::widget::column(__children).spacing(::ui_lang_runtime::bounded_spacing(4.0, __child_count)).padding(::ui_lang_runtime::bounded_padding(4.0, 0.0, 4.0, 0.0)).width(::iced::Fill).align_x(::iced::alignment::Horizontal::Center); let __content = ::iced::widget::container(__layout); let __layout_content: __IceElement<'_, __DucktapeMessage> = __content.into(); let __a11y_key = format!("{}/@layout:581", __ice_use_scope); ::ui_lang_runtime::accessible(__layout_content, ::ui_lang_runtime::StableId::new(&__a11y_key), ::ui_lang_runtime::Role::GenericContainer).logical_id_maybe(::core::cfg!(test).then_some(__a11y_key)).into() };
#[cfg(test)]
drop(__ice_render_source);
__ice_rendered
}; ::iced::widget::container(__button_inner).width(::iced::Fill).align_x(::iced::alignment::Horizontal::Center).into() }; let __button = ::iced::widget::button(__button_content).width(58.0 as f32).padding(0.0 as f32).on_press_maybe(if __disabled { None } else { Some(__activate.clone()) }).style(move |__theme, __status| { let mut __style = ::iced::widget::button::primary(__theme, __status); let __background: Option<::iced::Color> = match __status { ::iced::widget::button::Status::Hovered => Some(__ice_palette.colors[14]), ::iced::widget::button::Status::Pressed => Some(__ice_palette.colors[39]), ::iced::widget::button::Status::Disabled => Some(::iced::Color::from_rgba8(0, 0, 0, 0.000000)), _ => Some(::iced::Color::from_rgba8(0, 0, 0, 0.000000)) }; if let Some(__background) = __background { __style.background = Some(::iced::Background::Color(__background)); } __style.text_color = __ice_palette.colors[4]; __style.border.radius = 7.0.into(); __style.background = ::std::option::Option::Some(::iced::Background::Color(__ice_palette.colors[91])); __style.text_color = __ice_palette.colors[4]; __style.border.color = ::iced::Color::from_rgba8(0, 0, 0, 0.000000); __style.border.width = 1.0 as f32; __style.border.radius = ::iced::border::Radius { top_left: ((10.0) as f32).max(0.0).min(f32::MAX), top_right: ((10.0) as f32).max(0.0).min(f32::MAX), bottom_right: ((10.0) as f32).max(0.0).min(f32::MAX), bottom_left: ((10.0) as f32).max(0.0).min(f32::MAX) }; match __status { ::iced::widget::button::Status::Hovered => { __style.background = ::std::option::Option::Some(::iced::Background::Color(__ice_palette.colors[58])); __style.text_color = __ice_palette.colors[4]; } ::iced::widget::button::Status::Pressed => { __style.background = ::std::option::Option::Some(::iced::Background::Color(__ice_palette.colors[91])); __style.text_color = __ice_palette.colors[4]; } _ => {} } if matches!(__status, ::iced::widget::button::Status::Disabled) { if let Some(::iced::Background::Color(mut __color)) = __style.background { __color.a *= 0.5; __style.background = Some(::iced::Background::Color(__color)); } __style.text_color.a *= 0.5; } __style }); ::ui_lang_runtime::accessible(__button, __a11y_id, ::ui_lang_runtime::Role::Button).logical_id_maybe(::core::cfg!(test).then_some(&*__a11y_key)).focus_id(::iced::widget::Id::from(__a11y_key)).label(__ice_arg_0.title.to_owned()).checked(__ice_arg_0.active).disabled(__disabled).on_activate_maybe(if __disabled { None } else { Some(__activate) }).focus_ring(__ice_palette.colors[42], 7.0).into() };
#[cfg(test)]
drop(__ice_render_source);
__ice_rendered
}); } if (!__ice_arg_0.active) { __children.push({
#[cfg(test)]
let __ice_render_source_location = ::ui_lang_runtime::testing::Location::new("components/shell.ice", 602, 1, "rendered view node");
#[cfg(test)]
let __ice_render_source = ::ui_lang_runtime::testing::push_render_source(__ice_render_source_location);
let __ice_rendered: __IceElement<'_, __DucktapeMessage> = { let __a11y_key = format!("{}/@button:602", __ice_use_scope); let __a11y_id = ::ui_lang_runtime::StableId::new(&__a11y_key); let __disabled = false; let __activate = (__ice_cb_0)(__ice_arg_0.id.clone()); let __button_content: __IceElement<'_, __DucktapeMessage> = { let __button_inner: __IceElement<'_, __DucktapeMessage> = {
#[cfg(test)]
let __ice_render_source_location = ::ui_lang_runtime::testing::Location::new("components/shell.ice", 609, 1, "rendered view node");
#[cfg(test)]
let __ice_render_source = ::ui_lang_runtime::testing::push_render_source(__ice_render_source_location);
let __ice_rendered: __IceElement<'_, __DucktapeMessage> = { let mut __children: ::std::vec::Vec<__IceElement<'_, __DucktapeMessage>> = ::std::vec::Vec::new(); __children.push({
#[cfg(test)]
let __ice_render_source_location = ::ui_lang_runtime::testing::Location::new("components/shell.ice", 615, 1, "rendered view node");
#[cfg(test)]
let __ice_render_source = ::ui_lang_runtime::testing::push_render_source(__ice_render_source_location);
let __ice_rendered: __IceElement<'_, __DucktapeMessage> = ::ui_lang_runtime::grow_stack(|| self.__ice_component_use_109(__ice_palette, format!("{}/Icon@3535", __ice_use_scope), __ice_arg_0.icon.to_owned()));
#[cfg(test)]
drop(__ice_render_source);
__ice_rendered
}); __children.push({
#[cfg(test)]
let __ice_render_source_location = ::ui_lang_runtime::testing::Location::new("components/shell.ice", 620, 1, "rendered view node");
#[cfg(test)]
let __ice_render_source = ::ui_lang_runtime::testing::push_render_source(__ice_render_source_location);
let __ice_rendered: __IceElement<'_, __DucktapeMessage> = { let __a11y_key = format!("{}/@text:620", __ice_use_scope); let __text_value = (__ice_arg_0.title).to_string(); let __text = ::iced::widget::text(__text_value.clone()).size(((9.5) as f32).max(f32::EPSILON).min(f32::MAX)).wrapping(::iced::widget::text::Wrapping::None).font(::iced::Font { family: ::iced::font::Family::Name("Geist"), weight: ::iced::font::Weight::Semibold, stretch: ::iced::font::Stretch::Normal, style: ::iced::font::Style::Normal }).color(__ice_palette.colors[70]); let __text = ::ui_lang_runtime::selectable_text(__text); ::ui_lang_runtime::accessible(__text, ::ui_lang_runtime::StableId::new(&__a11y_key), ::ui_lang_runtime::Role::Label).logical_id_maybe(::core::cfg!(test).then_some(__a11y_key)).value(__text_value).into() };
#[cfg(test)]
drop(__ice_render_source);
__ice_rendered
}); let __child_count = __children.len(); let __children = __children.into_iter().map(|__child| ::ui_lang_runtime::bounded_fill_element(__child, __child_count, false)).collect::<::std::vec::Vec<_>>(); let __layout = ::iced::widget::column(__children).spacing(::ui_lang_runtime::bounded_spacing(4.0, __child_count)).padding(::ui_lang_runtime::bounded_padding(4.0, 0.0, 4.0, 0.0)).width(::iced::Fill).align_x(::iced::alignment::Horizontal::Center); let __content = ::iced::widget::container(__layout); let __layout_content: __IceElement<'_, __DucktapeMessage> = __content.into(); let __a11y_key = format!("{}/@layout:609", __ice_use_scope); ::ui_lang_runtime::accessible(__layout_content, ::ui_lang_runtime::StableId::new(&__a11y_key), ::ui_lang_runtime::Role::GenericContainer).logical_id_maybe(::core::cfg!(test).then_some(__a11y_key)).into() };
#[cfg(test)]
drop(__ice_render_source);
__ice_rendered
}; ::iced::widget::container(__button_inner).width(::iced::Fill).align_x(::iced::alignment::Horizontal::Center).into() }; let __button = ::iced::widget::button(__button_content).width(58.0 as f32).padding(0.0 as f32).on_press_maybe(if __disabled { None } else { Some(__activate.clone()) }).style(move |__theme, __status| { let mut __style = ::iced::widget::button::primary(__theme, __status); let __background: Option<::iced::Color> = match __status { ::iced::widget::button::Status::Hovered => Some(__ice_palette.colors[14]), ::iced::widget::button::Status::Pressed => Some(__ice_palette.colors[39]), ::iced::widget::button::Status::Disabled => Some(::iced::Color::from_rgba8(0, 0, 0, 0.000000)), _ => Some(::iced::Color::from_rgba8(0, 0, 0, 0.000000)) }; if let Some(__background) = __background { __style.background = Some(::iced::Background::Color(__background)); } __style.text_color = __ice_palette.colors[4]; __style.border.radius = 7.0.into(); __style.background = ::std::option::Option::Some(::iced::Background::Color(::iced::Color::from_rgba8(0, 0, 0, 0.000000))); __style.text_color = __ice_palette.colors[5]; __style.border.color = ::iced::Color::from_rgba8(0, 0, 0, 0.000000); __style.border.width = 1.0 as f32; __style.border.radius = ::iced::border::Radius { top_left: ((10.0) as f32).max(0.0).min(f32::MAX), top_right: ((10.0) as f32).max(0.0).min(f32::MAX), bottom_right: ((10.0) as f32).max(0.0).min(f32::MAX), bottom_left: ((10.0) as f32).max(0.0).min(f32::MAX) }; match __status { ::iced::widget::button::Status::Hovered => { __style.background = ::std::option::Option::Some(::iced::Background::Color(__ice_palette.colors[58])); __style.text_color = __ice_palette.colors[4]; } ::iced::widget::button::Status::Pressed => { __style.background = ::std::option::Option::Some(::iced::Background::Color(__ice_palette.colors[56])); __style.text_color = __ice_palette.colors[4]; } _ => {} } if matches!(__status, ::iced::widget::button::Status::Disabled) { if let Some(::iced::Background::Color(mut __color)) = __style.background { __color.a *= 0.5; __style.background = Some(::iced::Background::Color(__color)); } __style.text_color.a *= 0.5; } __style }); ::ui_lang_runtime::accessible(__button, __a11y_id, ::ui_lang_runtime::Role::Button).logical_id_maybe(::core::cfg!(test).then_some(&*__a11y_key)).focus_id(::iced::widget::Id::from(__a11y_key)).label(__ice_arg_0.title.to_owned()).checked(__ice_arg_0.active).disabled(__disabled).on_activate_maybe(if __disabled { None } else { Some(__activate) }).focus_ring(__ice_palette.colors[42], 7.0).into() };
#[cfg(test)]
drop(__ice_render_source);
__ice_rendered
}); } if __ice_arg_0.live { __children.push({
#[cfg(test)]
let __ice_render_source_location = ::ui_lang_runtime::testing::Location::new("components/shell.ice", 630, 1, "rendered view node");
#[cfg(test)]
let __ice_render_source = ::ui_lang_runtime::testing::push_render_source(__ice_render_source_location);
let __ice_rendered: __IceElement<'_, __DucktapeMessage> = { let __pin_content: __IceElement<'_, __DucktapeMessage> = {
#[cfg(test)]
let __ice_render_source_location = ::ui_lang_runtime::testing::Location::new("components/shell.ice", 631, 1, "rendered view node");
#[cfg(test)]
let __ice_render_source = ::ui_lang_runtime::testing::push_render_source(__ice_render_source_location);
let __ice_rendered: __IceElement<'_, __DucktapeMessage> = { let __a11y_key = format!("{}/@container:631", __ice_use_scope); let __container_content: __IceElement<'_, __DucktapeMessage> = {
#[cfg(test)]
let __ice_render_source_location = ::ui_lang_runtime::testing::Location::new("components/shell.ice", 636, 1, "rendered view node");
#[cfg(test)]
let __ice_render_source = ::ui_lang_runtime::testing::push_render_source(__ice_render_source_location);
let __ice_rendered: __IceElement<'_, __DucktapeMessage> = { let __ice_component_050756c7365446f74_scope_3556 = format!("{}/PulseDot@3556", __ice_use_scope); ::ui_lang_runtime::rev_memo(420u64, (__ice_palette.name), ::ui_lang_runtime::grow_stack(|| self.__ice_component_use_110(__ice_palette, __ice_component_050756c7365446f74_scope_3556))).into() };
#[cfg(test)]
drop(__ice_render_source);
__ice_rendered
}; let __container = {  ::iced::widget::container(__container_content).padding(::ui_lang_runtime::bounded_padding(1.5, 1.5, 1.5, 1.5)).style(move |__theme| { let mut __style = ::iced::widget::container::Style::default(); __style.background = ::std::option::Option::Some(::iced::Background::Color(__ice_palette.colors[53])); __style.border.radius = ::iced::border::Radius { top_left: ((5.0) as f32).max(0.0).min(f32::MAX), top_right: ((5.0) as f32).max(0.0).min(f32::MAX), bottom_right: ((5.0) as f32).max(0.0).min(f32::MAX), bottom_left: ((5.0) as f32).max(0.0).min(f32::MAX) }; __style }) }; ::ui_lang_runtime::accessible(__container, ::ui_lang_runtime::StableId::new(&__a11y_key), ::ui_lang_runtime::Role::GenericContainer).logical_id_maybe(::core::cfg!(test).then_some(__a11y_key)).into() };
#[cfg(test)]
drop(__ice_render_source);
__ice_rendered
}; ::iced::widget::pin(__pin_content).x(36.0 as f32).y(6.0 as f32).into() };
#[cfg(test)]
drop(__ice_render_source);
__ice_rendered
}); } if ((__ice_arg_0.badge > 0) && (__ice_arg_0.badge < 10)) { __children.push({
#[cfg(test)]
let __ice_render_source_location = ::ui_lang_runtime::testing::Location::new("components/shell.ice", 640, 1, "rendered view node");
#[cfg(test)]
let __ice_render_source = ::ui_lang_runtime::testing::push_render_source(__ice_render_source_location);
let __ice_rendered: __IceElement<'_, __DucktapeMessage> = { let __pin_content: __IceElement<'_, __DucktapeMessage> = {
#[cfg(test)]
let __ice_render_source_location = ::ui_lang_runtime::testing::Location::new("components/shell.ice", 641, 1, "rendered view node");
#[cfg(test)]
let __ice_render_source = ::ui_lang_runtime::testing::push_render_source(__ice_render_source_location);
let __ice_rendered: __IceElement<'_, __DucktapeMessage> = { let __a11y_key = format!("{}/@container:641", __ice_use_scope); let __container_content: __IceElement<'_, __DucktapeMessage> = {
#[cfg(test)]
let __ice_render_source_location = ::ui_lang_runtime::testing::Location::new("components/shell.ice", 651, 1, "rendered view node");
#[cfg(test)]
let __ice_render_source = ::ui_lang_runtime::testing::push_render_source(__ice_render_source_location);
let __ice_rendered: __IceElement<'_, __DucktapeMessage> = { let __a11y_key = format!("{}/@text:651", __ice_use_scope); let __text_value = (__ice_arg_0.badge).to_string(); let __text = ::iced::widget::text(__text_value.clone()).size(((9.0) as f32).max(f32::EPSILON).min(f32::MAX)).wrapping(::iced::widget::text::Wrapping::None).font(::iced::Font { family: ::iced::font::Family::Name("Geist Mono"), weight: ::iced::font::Weight::Semibold, stretch: ::iced::font::Stretch::Normal, style: ::iced::font::Style::Normal }).color(__ice_palette.colors[17]); let __text = ::ui_lang_runtime::selectable_text(__text); ::ui_lang_runtime::accessible(__text, ::ui_lang_runtime::StableId::new(&__a11y_key), ::ui_lang_runtime::Role::Label).logical_id_maybe(::core::cfg!(test).then_some(__a11y_key)).value(__text_value).into() };
#[cfg(test)]
drop(__ice_render_source);
__ice_rendered
}; let __container = {  ::iced::widget::container(__container_content).width(15.0 as f32).height(15.0 as f32).align_x(::iced::alignment::Horizontal::Center).align_y(::iced::alignment::Vertical::Center).style(move |__theme| { let mut __style = ::iced::widget::container::Style::default(); __style.background = ::std::option::Option::Some(::iced::Background::Color(__ice_palette.colors[16])); __style.border.color = __ice_palette.colors[53]; __style.border.width = 1.5 as f32; __style.border.radius = ::iced::border::Radius { top_left: ((8.0) as f32).max(0.0).min(f32::MAX), top_right: ((8.0) as f32).max(0.0).min(f32::MAX), bottom_right: ((8.0) as f32).max(0.0).min(f32::MAX), bottom_left: ((8.0) as f32).max(0.0).min(f32::MAX) }; __style }) }; ::ui_lang_runtime::accessible(__container, ::ui_lang_runtime::StableId::new(&__a11y_key), ::ui_lang_runtime::Role::GenericContainer).logical_id_maybe(::core::cfg!(test).then_some(__a11y_key)).into() };
#[cfg(test)]
drop(__ice_render_source);
__ice_rendered
}; ::iced::widget::pin(__pin_content).x(32.0 as f32).y(6.0 as f32).into() };
#[cfg(test)]
drop(__ice_render_source);
__ice_rendered
}); } if (__ice_arg_0.badge > 9) { __children.push({
#[cfg(test)]
let __ice_render_source_location = ::ui_lang_runtime::testing::Location::new("components/shell.ice", 658, 1, "rendered view node");
#[cfg(test)]
let __ice_render_source = ::ui_lang_runtime::testing::push_render_source(__ice_render_source_location);
let __ice_rendered: __IceElement<'_, __DucktapeMessage> = { let __pin_content: __IceElement<'_, __DucktapeMessage> = {
#[cfg(test)]
let __ice_render_source_location = ::ui_lang_runtime::testing::Location::new("components/shell.ice", 659, 1, "rendered view node");
#[cfg(test)]
let __ice_render_source = ::ui_lang_runtime::testing::push_render_source(__ice_render_source_location);
let __ice_rendered: __IceElement<'_, __DucktapeMessage> = { let __a11y_key = format!("{}/@container:659", __ice_use_scope); let __container_content: __IceElement<'_, __DucktapeMessage> = {
#[cfg(test)]
let __ice_render_source_location = ::ui_lang_runtime::testing::Location::new("components/shell.ice", 669, 1, "rendered view node");
#[cfg(test)]
let __ice_render_source = ::ui_lang_runtime::testing::push_render_source(__ice_render_source_location);
let __ice_rendered: __IceElement<'_, __DucktapeMessage> = { let __a11y_key = format!("{}/@text:669", __ice_use_scope); let __text_value = (__ice_arg_0.badge).to_string(); let __text = ::iced::widget::text(__text_value.clone()).size(((9.0) as f32).max(f32::EPSILON).min(f32::MAX)).wrapping(::iced::widget::text::Wrapping::None).font(::iced::Font { family: ::iced::font::Family::Name("Geist Mono"), weight: ::iced::font::Weight::Semibold, stretch: ::iced::font::Stretch::Normal, style: ::iced::font::Style::Normal }).color(__ice_palette.colors[17]); let __text = ::ui_lang_runtime::selectable_text(__text); ::ui_lang_runtime::accessible(__text, ::ui_lang_runtime::StableId::new(&__a11y_key), ::ui_lang_runtime::Role::Label).logical_id_maybe(::core::cfg!(test).then_some(__a11y_key)).value(__text_value).into() };
#[cfg(test)]
drop(__ice_render_source);
__ice_rendered
}; let __container = {  ::iced::widget::container(__container_content).width(22.0 as f32).height(15.0 as f32).align_x(::iced::alignment::Horizontal::Center).align_y(::iced::alignment::Vertical::Center).style(move |__theme| { let mut __style = ::iced::widget::container::Style::default(); __style.background = ::std::option::Option::Some(::iced::Background::Color(__ice_palette.colors[16])); __style.border.color = __ice_palette.colors[53]; __style.border.width = 1.5 as f32; __style.border.radius = ::iced::border::Radius { top_left: ((8.0) as f32).max(0.0).min(f32::MAX), top_right: ((8.0) as f32).max(0.0).min(f32::MAX), bottom_right: ((8.0) as f32).max(0.0).min(f32::MAX), bottom_left: ((8.0) as f32).max(0.0).min(f32::MAX) }; __style }) }; ::ui_lang_runtime::accessible(__container, ::ui_lang_runtime::StableId::new(&__a11y_key), ::ui_lang_runtime::Role::GenericContainer).logical_id_maybe(::core::cfg!(test).then_some(__a11y_key)).into() };
#[cfg(test)]
drop(__ice_render_source);
__ice_rendered
}; ::iced::widget::pin(__pin_content).x(25.0 as f32).y(6.0 as f32).into() };
#[cfg(test)]
drop(__ice_render_source);
__ice_rendered
}); } let __layout = ::ui_lang_runtime::zstack(__children).width(58.0 as f32); let __content = ::iced::widget::container(__layout); let __layout_content: __IceElement<'_, __DucktapeMessage> = __content.into(); let __a11y_key = __ice_node_scope.as_str(); ::ui_lang_runtime::accessible(__layout_content, ::ui_lang_runtime::StableId::new(&__a11y_key), ::ui_lang_runtime::Role::GenericContainer).logical_id_maybe(::core::cfg!(test).then_some(__a11y_key)).into() } };
#[cfg(test)]
drop(__ice_render_source);
__ice_rendered
}; __component_content }
#[cfg(test)]
pub(super) fn __ice_component_use_117(&self, __ice_palette: __IcePalette, __ice_use_scope: ::std::string::String, __ice_cb_0: impl Fn(ShellTab) -> __DucktapeMessage + Clone + 'static) -> __IceElement<'_, __DucktapeMessage> { let __component_content: __IceElement<'_, __DucktapeMessage> = {
#[cfg(test)]
let __ice_render_source_location = ::ui_lang_runtime::testing::Location::new("components/shell.ice", 679, 1, "rendered view node");
#[cfg(test)]
let __ice_render_source = ::ui_lang_runtime::testing::push_render_source(__ice_render_source_location);
let __ice_rendered: __IceElement<'_, __DucktapeMessage> = { let __ice_node_scope = format!("{}/root", __ice_use_scope); { let __a11y_key = __ice_node_scope.as_str(); let __container_content: __IceElement<'_, __DucktapeMessage> = {
#[cfg(test)]
let __ice_render_source_location = ::ui_lang_runtime::testing::Location::new("components/shell.ice", 687, 1, "rendered view node");
#[cfg(test)]
let __ice_render_source = ::ui_lang_runtime::testing::push_render_source(__ice_render_source_location);
let __ice_rendered: __IceElement<'_, __DucktapeMessage> = { let mut __children: ::std::vec::Vec<__IceElement<'_, __DucktapeMessage>> = ::std::vec::Vec::new(); __children.push({
#[cfg(test)]
let __ice_render_source_location = ::ui_lang_runtime::testing::Location::new("components/shell.ice", 693, 1, "rendered view node");
#[cfg(test)]
let __ice_render_source = ::ui_lang_runtime::testing::push_render_source(__ice_render_source_location);
let __ice_rendered: __IceElement<'_, __DucktapeMessage> = { let __ice_component_brand_scope_3613 = format!("{}/Brand@3613", __ice_use_scope); ::ui_lang_runtime::rev_memo(431u64, (__ice_palette.name), ::ui_lang_runtime::grow_stack(|| self.__ice_component_use_107(__ice_palette, __ice_component_brand_scope_3613))).into() };
#[cfg(test)]
drop(__ice_render_source);
__ice_rendered
}); __children.push({
#[cfg(test)]
let __ice_render_source_location = ::ui_lang_runtime::testing::Location::new("components/shell.ice", 694, 1, "rendered view node");
#[cfg(test)]
let __ice_render_source = ::ui_lang_runtime::testing::push_render_source(__ice_render_source_location);
let __ice_rendered: __IceElement<'_, __DucktapeMessage> = ::iced::widget::space().width(1.0 as f32).height(9.0 as f32).into();
#[cfg(test)]
drop(__ice_render_source);
__ice_rendered
}); __children.push({
#[cfg(test)]
let __ice_render_source_location = ::ui_lang_runtime::testing::Location::new("components/shell.ice", 695, 1, "rendered view node");
#[cfg(test)]
let __ice_render_source = ::ui_lang_runtime::testing::push_render_source(__ice_render_source_location);
let __ice_rendered: __IceElement<'_, __DucktapeMessage> = { let __a11y_key = format!("{}/@layout:695", __ice_use_scope); let __scroll_content: __IceElement<'_, __DucktapeMessage> = {
#[cfg(test)]
let __ice_render_source_location = ::ui_lang_runtime::testing::Location::new("components/shell.ice", 701, 1, "rendered view node");
#[cfg(test)]
let __ice_render_source = ::ui_lang_runtime::testing::push_render_source(__ice_render_source_location);
let __ice_rendered: __IceElement<'_, __DucktapeMessage> = { let mut __children: ::std::vec::Vec<__IceElement<'_, __DucktapeMessage>> = ::std::vec::Vec::new(); for (__ice_index, item) in crate::backend::shell_nav(self.shell_tab.clone(), 0, false).iter().enumerate() { let __for_scope = format!("{}/@for:3626({})", __ice_use_scope, __ice_index); __children.push({
#[cfg(test)]
let __ice_render_source_location = ::ui_lang_runtime::testing::Location::new("components/shell.ice", 707, 1, "rendered view node");
#[cfg(test)]
let __ice_render_source = ::ui_lang_runtime::testing::push_render_source(__ice_render_source_location);
let __ice_rendered: __IceElement<'_, __DucktapeMessage> = ::ui_lang_runtime::grow_stack(|| self.__ice_component_use_111(__ice_palette, format!("{}/RailButton@3627", __for_scope), (__ice_cb_0).clone(), item.clone()));
#[cfg(test)]
drop(__ice_render_source);
__ice_rendered
}); } let __child_count = __children.len(); let __children = __children.into_iter().map(|__child| ::ui_lang_runtime::bounded_fill_element(__child, __child_count, false)).collect::<::std::vec::Vec<_>>(); let __layout = ::iced::widget::column(__children).spacing(::ui_lang_runtime::bounded_spacing(2.0, __child_count)).width(::iced::Fill).align_x(::iced::alignment::Horizontal::Center); let __content = ::iced::widget::container(__layout); let __layout_content: __IceElement<'_, __DucktapeMessage> = __content.into(); let __a11y_key = format!("{}/@layout:701", __ice_use_scope); ::ui_lang_runtime::accessible(__layout_content, ::ui_lang_runtime::StableId::new(&__a11y_key), ::ui_lang_runtime::Role::GenericContainer).logical_id_maybe(::core::cfg!(test).then_some(__a11y_key)).into() };
#[cfg(test)]
drop(__ice_render_source);
__ice_rendered
}; let __layout = ::iced::widget::scrollable(__scroll_content).direction(::iced::widget::scrollable::Direction::Vertical(::iced::widget::scrollable::Scrollbar::hidden())).anchor_x(::iced::widget::scrollable::Anchor::Start).anchor_y(::iced::widget::scrollable::Anchor::Start).width(::iced::Fill).height(::iced::Fill); ::ui_lang_runtime::accessible(__layout, ::ui_lang_runtime::StableId::new(&__a11y_key), ::ui_lang_runtime::Role::GenericContainer).logical_id_maybe(::core::cfg!(test).then_some(__a11y_key)).into() };
#[cfg(test)]
drop(__ice_render_source);
__ice_rendered
}); if (self.shell_tab == ShellTab::Settings) { __children.push({
#[cfg(test)]
let __ice_render_source_location = ::ui_lang_runtime::testing::Location::new("components/shell.ice", 711, 1, "rendered view node");
#[cfg(test)]
let __ice_render_source = ::ui_lang_runtime::testing::push_render_source(__ice_render_source_location);
let __ice_rendered: __IceElement<'_, __DucktapeMessage> = { let __a11y_key = format!("{}/@button:711", __ice_use_scope); let __a11y_id = ::ui_lang_runtime::StableId::new(&__a11y_key); let __disabled = false; let __activate = (__ice_cb_0)(ShellTab::Settings); let __button_content: __IceElement<'_, __DucktapeMessage> = {
#[cfg(test)]
let __ice_render_source_location = ::ui_lang_runtime::testing::Location::new("components/shell.ice", 717, 1, "rendered view node");
#[cfg(test)]
let __ice_render_source = ::ui_lang_runtime::testing::push_render_source(__ice_render_source_location);
let __ice_rendered: __IceElement<'_, __DucktapeMessage> = ::ui_lang_runtime::grow_stack(|| self.__ice_component_use_112(__ice_palette, format!("{}/Icon@3637", __ice_use_scope)));
#[cfg(test)]
drop(__ice_render_source);
__ice_rendered
}; let __button = ::iced::widget::button(__button_content).padding(8.0 as f32).on_press_maybe(if __disabled { None } else { Some(__activate.clone()) }).style(move |__theme, __status| { let mut __style = ::iced::widget::button::primary(__theme, __status); let __background: Option<::iced::Color> = match __status { ::iced::widget::button::Status::Hovered => Some(__ice_palette.colors[14]), ::iced::widget::button::Status::Pressed => Some(__ice_palette.colors[39]), ::iced::widget::button::Status::Disabled => Some(::iced::Color::from_rgba8(0, 0, 0, 0.000000)), _ => Some(::iced::Color::from_rgba8(0, 0, 0, 0.000000)) }; if let Some(__background) = __background { __style.background = Some(::iced::Background::Color(__background)); } __style.text_color = __ice_palette.colors[4]; __style.border.radius = 7.0.into(); __style.background = ::std::option::Option::Some(::iced::Background::Color(__ice_palette.colors[91])); __style.text_color = __ice_palette.colors[4]; __style.border.color = ::iced::Color::from_rgba8(0, 0, 0, 0.000000); __style.border.width = 1.0 as f32; __style.border.radius = ::iced::border::Radius { top_left: ((9.0) as f32).max(0.0).min(f32::MAX), top_right: ((9.0) as f32).max(0.0).min(f32::MAX), bottom_right: ((9.0) as f32).max(0.0).min(f32::MAX), bottom_left: ((9.0) as f32).max(0.0).min(f32::MAX) }; match __status { ::iced::widget::button::Status::Hovered => { __style.background = ::std::option::Option::Some(::iced::Background::Color(__ice_palette.colors[58])); __style.text_color = __ice_palette.colors[4]; } ::iced::widget::button::Status::Pressed => { __style.background = ::std::option::Option::Some(::iced::Background::Color(__ice_palette.colors[91])); __style.text_color = __ice_palette.colors[4]; } _ => {} } if matches!(__status, ::iced::widget::button::Status::Disabled) { if let Some(::iced::Background::Color(mut __color)) = __style.background { __color.a *= 0.5; __style.background = Some(::iced::Background::Color(__color)); } __style.text_color.a *= 0.5; } __style }); ::ui_lang_runtime::accessible(__button, __a11y_id, ::ui_lang_runtime::Role::Button).logical_id_maybe(::core::cfg!(test).then_some(&*__a11y_key)).focus_id(::iced::widget::Id::from(__a11y_key)).label("Settings".to_owned()).checked(true).disabled(__disabled).on_activate_maybe(if __disabled { None } else { Some(__activate) }).focus_ring(__ice_palette.colors[42], 7.0).into() };
#[cfg(test)]
drop(__ice_render_source);
__ice_rendered
}); } if (self.shell_tab != ShellTab::Settings) { __children.push({
#[cfg(test)]
let __ice_render_source_location = ::ui_lang_runtime::testing::Location::new("components/shell.ice", 726, 1, "rendered view node");
#[cfg(test)]
let __ice_render_source = ::ui_lang_runtime::testing::push_render_source(__ice_render_source_location);
let __ice_rendered: __IceElement<'_, __DucktapeMessage> = { let __a11y_key = format!("{}/@button:726", __ice_use_scope); let __a11y_id = ::ui_lang_runtime::StableId::new(&__a11y_key); let __disabled = false; let __activate = (__ice_cb_0)(ShellTab::Settings); let __button_content: __IceElement<'_, __DucktapeMessage> = {
#[cfg(test)]
let __ice_render_source_location = ::ui_lang_runtime::testing::Location::new("components/shell.ice", 732, 1, "rendered view node");
#[cfg(test)]
let __ice_render_source = ::ui_lang_runtime::testing::push_render_source(__ice_render_source_location);
let __ice_rendered: __IceElement<'_, __DucktapeMessage> = ::ui_lang_runtime::grow_stack(|| self.__ice_component_use_113(__ice_palette, format!("{}/Icon@3652", __ice_use_scope)));
#[cfg(test)]
drop(__ice_render_source);
__ice_rendered
}; let __button = ::iced::widget::button(__button_content).padding(8.0 as f32).on_press_maybe(if __disabled { None } else { Some(__activate.clone()) }).style(move |__theme, __status| { let mut __style = ::iced::widget::button::primary(__theme, __status); let __background: Option<::iced::Color> = match __status { ::iced::widget::button::Status::Hovered => Some(__ice_palette.colors[14]), ::iced::widget::button::Status::Pressed => Some(__ice_palette.colors[39]), ::iced::widget::button::Status::Disabled => Some(::iced::Color::from_rgba8(0, 0, 0, 0.000000)), _ => Some(::iced::Color::from_rgba8(0, 0, 0, 0.000000)) }; if let Some(__background) = __background { __style.background = Some(::iced::Background::Color(__background)); } __style.text_color = __ice_palette.colors[4]; __style.border.radius = 7.0.into(); __style.background = ::std::option::Option::Some(::iced::Background::Color(::iced::Color::from_rgba8(0, 0, 0, 0.000000))); __style.text_color = __ice_palette.colors[5]; __style.border.color = ::iced::Color::from_rgba8(0, 0, 0, 0.000000); __style.border.width = 1.0 as f32; __style.border.radius = ::iced::border::Radius { top_left: ((9.0) as f32).max(0.0).min(f32::MAX), top_right: ((9.0) as f32).max(0.0).min(f32::MAX), bottom_right: ((9.0) as f32).max(0.0).min(f32::MAX), bottom_left: ((9.0) as f32).max(0.0).min(f32::MAX) }; match __status { ::iced::widget::button::Status::Hovered => { __style.background = ::std::option::Option::Some(::iced::Background::Color(__ice_palette.colors[58])); __style.text_color = __ice_palette.colors[4]; } ::iced::widget::button::Status::Pressed => { __style.background = ::std::option::Option::Some(::iced::Background::Color(__ice_palette.colors[56])); __style.text_color = __ice_palette.colors[4]; } _ => {} } if matches!(__status, ::iced::widget::button::Status::Disabled) { if let Some(::iced::Background::Color(mut __color)) = __style.background { __color.a *= 0.5; __style.background = Some(::iced::Background::Color(__color)); } __style.text_color.a *= 0.5; } __style }); ::ui_lang_runtime::accessible(__button, __a11y_id, ::ui_lang_runtime::Role::Button).logical_id_maybe(::core::cfg!(test).then_some(&*__a11y_key)).focus_id(::iced::widget::Id::from(__a11y_key)).label("Settings".to_owned()).checked(false).disabled(__disabled).on_activate_maybe(if __disabled { None } else { Some(__activate) }).focus_ring(__ice_palette.colors[42], 7.0).into() };
#[cfg(test)]
drop(__ice_render_source);
__ice_rendered
}); } __children.push({
#[cfg(test)]
let __ice_render_source_location = ::ui_lang_runtime::testing::Location::new("components/shell.ice", 740, 1, "rendered view node");
#[cfg(test)]
let __ice_render_source = ::ui_lang_runtime::testing::push_render_source(__ice_render_source_location);
let __ice_rendered: __IceElement<'_, __DucktapeMessage> = ::iced::widget::space().width(1.0 as f32).height(6.0 as f32).into();
#[cfg(test)]
drop(__ice_render_source);
__ice_rendered
}); __children.push({
#[cfg(test)]
let __ice_render_source_location = ::ui_lang_runtime::testing::Location::new("components/shell.ice", 744, 1, "rendered view node");
#[cfg(test)]
let __ice_render_source = ::ui_lang_runtime::testing::push_render_source(__ice_render_source_location);
let __ice_rendered: __IceElement<'_, __DucktapeMessage> = { let __a11y_key = format!("{}/@button:744", __ice_use_scope); let __a11y_id = ::ui_lang_runtime::StableId::new(&__a11y_key); let __disabled = false; let __activate = (__ice_cb_0)(ShellTab::Settings); let __button_content: __IceElement<'_, __DucktapeMessage> = {
#[cfg(test)]
let __ice_render_source_location = ::ui_lang_runtime::testing::Location::new("components/shell.ice", 749, 1, "rendered view node");
#[cfg(test)]
let __ice_render_source = ::ui_lang_runtime::testing::push_render_source(__ice_render_source_location);
let __ice_rendered: __IceElement<'_, __DucktapeMessage> = { let __a11y_key = format!("{}/@container:749", __ice_use_scope); let __container_content: __IceElement<'_, __DucktapeMessage> = {
#[cfg(test)]
let __ice_render_source_location = ::ui_lang_runtime::testing::Location::new("components/shell.ice", 754, 1, "rendered view node");
#[cfg(test)]
let __ice_render_source = ::ui_lang_runtime::testing::push_render_source(__ice_render_source_location);
let __ice_rendered: __IceElement<'_, __DucktapeMessage> = { let __ice_component_05072696e636970616c417661746172_scope_3674 = format!("{}/PrincipalAvatar@3674", __ice_use_scope); ::ui_lang_runtime::rev_memo(446u64, (__ice_palette.name), ::ui_lang_runtime::grow_stack(|| self.__ice_component_use_116(__ice_palette, __ice_component_05072696e636970616c417661746172_scope_3674))).into() };
#[cfg(test)]
drop(__ice_render_source);
__ice_rendered
}; let __container = {  ::iced::widget::container(__container_content).padding(::ui_lang_runtime::bounded_padding(1.0, 1.0, 1.0, 1.0)).style(move |__theme| { let mut __style = ::iced::widget::container::Style::default(); __style.background = ::std::option::Option::Some(::iced::Background::Color(__ice_palette.colors[93])); __style.border.radius = ::iced::border::Radius { top_left: ((16.5) as f32).max(0.0).min(f32::MAX), top_right: ((16.5) as f32).max(0.0).min(f32::MAX), bottom_right: ((16.5) as f32).max(0.0).min(f32::MAX), bottom_left: ((16.5) as f32).max(0.0).min(f32::MAX) }; __style }) }; ::ui_lang_runtime::accessible(__container, ::ui_lang_runtime::StableId::new(&__a11y_key), ::ui_lang_runtime::Role::GenericContainer).logical_id_maybe(::core::cfg!(test).then_some(__a11y_key)).into() };
#[cfg(test)]
drop(__ice_render_source);
__ice_rendered
}; let __button = ::iced::widget::button(__button_content).padding(0.0 as f32).on_press_maybe(if __disabled { None } else { Some(__activate.clone()) }).style(move |__theme, __status| { let mut __style = ::iced::widget::button::primary(__theme, __status); let __background: Option<::iced::Color> = match __status { ::iced::widget::button::Status::Hovered => Some(__ice_palette.colors[14]), ::iced::widget::button::Status::Pressed => Some(__ice_palette.colors[39]), ::iced::widget::button::Status::Disabled => Some(::iced::Color::from_rgba8(0, 0, 0, 0.000000)), _ => Some(::iced::Color::from_rgba8(0, 0, 0, 0.000000)) }; if let Some(__background) = __background { __style.background = Some(::iced::Background::Color(__background)); } __style.text_color = __ice_palette.colors[4]; __style.border.radius = 7.0.into(); __style.background = ::std::option::Option::Some(::iced::Background::Color(::iced::Color::from_rgba8(0, 0, 0, 0.000000))); __style.text_color = __ice_palette.colors[5]; __style.border.color = ::iced::Color::from_rgba8(0, 0, 0, 0.000000); __style.border.width = 1.0 as f32; __style.border.radius = ::iced::border::Radius { top_left: ((16.5) as f32).max(0.0).min(f32::MAX), top_right: ((16.5) as f32).max(0.0).min(f32::MAX), bottom_right: ((16.5) as f32).max(0.0).min(f32::MAX), bottom_left: ((16.5) as f32).max(0.0).min(f32::MAX) }; match __status { ::iced::widget::button::Status::Hovered => { __style.background = ::std::option::Option::Some(::iced::Background::Color(__ice_palette.colors[58])); __style.text_color = __ice_palette.colors[4]; } ::iced::widget::button::Status::Pressed => { __style.background = ::std::option::Option::Some(::iced::Background::Color(__ice_palette.colors[56])); __style.text_color = __ice_palette.colors[4]; } _ => {} } if matches!(__status, ::iced::widget::button::Status::Disabled) { if let Some(::iced::Background::Color(mut __color)) = __style.background { __color.a *= 0.5; __style.background = Some(::iced::Background::Color(__color)); } __style.text_color.a *= 0.5; } __style }); ::ui_lang_runtime::accessible(__button, __a11y_id, ::ui_lang_runtime::Role::Button).logical_id_maybe(::core::cfg!(test).then_some(&*__a11y_key)).focus_id(::iced::widget::Id::from(__a11y_key)).label("Account".to_owned()).disabled(__disabled).on_activate_maybe(if __disabled { None } else { Some(__activate) }).focus_ring(__ice_palette.colors[42], 7.0).into() };
#[cfg(test)]
drop(__ice_render_source);
__ice_rendered
}); let __child_count = __children.len(); let __children = __children.into_iter().map(|__child| ::ui_lang_runtime::bounded_fill_element(__child, __child_count, false)).collect::<::std::vec::Vec<_>>(); let __layout = ::iced::widget::column(__children).spacing(::ui_lang_runtime::bounded_spacing(0.0, __child_count)).width(::iced::Fill).height(::iced::Fill).align_x(::iced::alignment::Horizontal::Center); let __content = ::iced::widget::container(__layout); let __layout_content: __IceElement<'_, __DucktapeMessage> = __content.into(); let __a11y_key = format!("{}/@layout:687", __ice_use_scope); ::ui_lang_runtime::accessible(__layout_content, ::ui_lang_runtime::StableId::new(&__a11y_key), ::ui_lang_runtime::Role::GenericContainer).logical_id_maybe(::core::cfg!(test).then_some(__a11y_key)).into() };
#[cfg(test)]
drop(__ice_render_source);
__ice_rendered
}; let __container = {  ::iced::widget::container(__container_content).id(::iced::widget::Id::from(__ice_node_scope.clone())).padding(::ui_lang_runtime::bounded_padding(13.0, 0.0, 10.0, 0.0)).width(74.0 as f32).height(::iced::Fill).clip(true).style(move |__theme| { let mut __style = ::iced::widget::container::Style::default(); __style.background = ::std::option::Option::Some(::iced::Background::Color(__ice_palette.colors[53])); __style }) }; ::ui_lang_runtime::accessible(__container, ::ui_lang_runtime::StableId::new(&__a11y_key), ::ui_lang_runtime::Role::GenericContainer).logical_id_maybe(::core::cfg!(test).then_some(__a11y_key)).into() } };
#[cfg(test)]
drop(__ice_render_source);
__ice_rendered
}; __component_content }
#[cfg(test)]
pub(super) fn __ice_component_use_118(&self, __ice_palette: __IcePalette, __ice_use_scope: ::std::string::String) -> __IceElement<'_, __DucktapeMessage> { let __component_content: __IceElement<'_, __DucktapeMessage> = {
#[cfg(test)]
let __ice_render_source_location = ::ui_lang_runtime::testing::Location::new("components/shell.ice", 855, 1, "rendered view node");
#[cfg(test)]
let __ice_render_source = ::ui_lang_runtime::testing::push_render_source(__ice_render_source_location);
let __ice_rendered: __IceElement<'_, __DucktapeMessage> = { let __a11y_key = format!("{}/@container:855", __ice_use_scope); let __container_content: __IceElement<'_, __DucktapeMessage> = {
#[cfg(test)]
let __ice_render_source_location = ::ui_lang_runtime::testing::Location::new("components/shell.ice", 862, 1, "rendered view node");
#[cfg(test)]
let __ice_render_source = ::ui_lang_runtime::testing::push_render_source(__ice_render_source_location);
let __ice_rendered: __IceElement<'_, __DucktapeMessage> = { let mut __children: ::std::vec::Vec<__IceElement<'_, __DucktapeMessage>> = ::std::vec::Vec::new(); __children.push({
#[cfg(test)]
let __ice_render_source_location = ::ui_lang_runtime::testing::Location::new("components/shell.ice", 863, 1, "rendered view node");
#[cfg(test)]
let __ice_render_source = ::ui_lang_runtime::testing::push_render_source(__ice_render_source_location);
let __ice_rendered: __IceElement<'_, __DucktapeMessage> = { let mut __children: ::std::vec::Vec<__IceElement<'_, __DucktapeMessage>> = ::std::vec::Vec::new(); __children.push({
#[cfg(test)]
let __ice_render_source_location = ::ui_lang_runtime::testing::Location::new("components/shell.ice", 864, 1, "rendered view node");
#[cfg(test)]
let __ice_render_source = ::ui_lang_runtime::testing::push_render_source(__ice_render_source_location);
let __ice_rendered: __IceElement<'_, __DucktapeMessage> = { let __ice_node_scope = format!("{}/titlebar", __ice_use_scope); ::ui_lang_runtime::grow_stack(|| self.__ice_component_use_106(__ice_palette, __ice_node_scope.clone(), (move || __DucktapeMessage::SwitchNetwork).clone(), (move || __DucktapeMessage::ToggleBell).clone())) };
#[cfg(test)]
drop(__ice_render_source);
__ice_rendered
}); __children.push({
#[cfg(test)]
let __ice_render_source_location = ::ui_lang_runtime::testing::Location::new("components/shell.ice", 883, 1, "rendered view node");
#[cfg(test)]
let __ice_render_source = ::ui_lang_runtime::testing::push_render_source(__ice_render_source_location);
let __ice_rendered: __IceElement<'_, __DucktapeMessage> = { let mut __children: ::std::vec::Vec<__IceElement<'_, __DucktapeMessage>> = ::std::vec::Vec::new(); __children.push({
#[cfg(test)]
let __ice_render_source_location = ::ui_lang_runtime::testing::Location::new("components/shell.ice", 884, 1, "rendered view node");
#[cfg(test)]
let __ice_render_source = ::ui_lang_runtime::testing::push_render_source(__ice_render_source_location);
let __ice_rendered: __IceElement<'_, __DucktapeMessage> = { let __ice_node_scope = format!("{}/rail", __ice_use_scope); ::ui_lang_runtime::grow_stack(|| self.__ice_component_use_117(__ice_palette, __ice_node_scope.clone(), (move |__event_0| __DucktapeMessage::SelectShellTab(__event_0)).clone())) };
#[cfg(test)]
drop(__ice_render_source);
__ice_rendered
}); __children.push({
#[cfg(test)]
let __ice_render_source_location = ::ui_lang_runtime::testing::Location::new("components/shell.ice", 892, 1, "rendered view node");
#[cfg(test)]
let __ice_render_source = ::ui_lang_runtime::testing::push_render_source(__ice_render_source_location);
let __ice_rendered: __IceElement<'_, __DucktapeMessage> = { let __a11y_key = format!("{}/@container:892", __ice_use_scope); let __container_content: __IceElement<'_, __DucktapeMessage> = {
#[cfg(test)]
let __ice_render_source_location = ::ui_lang_runtime::testing::Location::new("components/shell.ice", 897, 1, "rendered view node");
#[cfg(test)]
let __ice_render_source = ::ui_lang_runtime::testing::push_render_source(__ice_render_source_location);
let __ice_rendered: __IceElement<'_, __DucktapeMessage> = ::iced::widget::space().width(1.0 as f32).height(1.0 as f32).into();
#[cfg(test)]
drop(__ice_render_source);
__ice_rendered
}; let __container = {  ::iced::widget::container(__container_content).width(1.0 as f32).height(::iced::Fill).style(move |__theme| { let mut __style = ::iced::widget::container::Style::default(); __style.background = ::std::option::Option::Some(::iced::Background::Color(__ice_palette.colors[60])); __style }) }; ::ui_lang_runtime::accessible(__container, ::ui_lang_runtime::StableId::new(&__a11y_key), ::ui_lang_runtime::Role::GenericContainer).logical_id_maybe(::core::cfg!(test).then_some(__a11y_key)).into() };
#[cfg(test)]
drop(__ice_render_source);
__ice_rendered
}); __children.push({
#[cfg(test)]
let __ice_render_source_location = ::ui_lang_runtime::testing::Location::new("components/shell.ice", 898, 1, "rendered view node");
#[cfg(test)]
let __ice_render_source = ::ui_lang_runtime::testing::push_render_source(__ice_render_source_location);
let __ice_rendered: __IceElement<'_, __DucktapeMessage> = { let __ice_node_scope = format!("{}/content", __ice_use_scope); { let __a11y_key = __ice_node_scope.as_str(); let __container_content: __IceElement<'_, __DucktapeMessage> = {
#[cfg(test)]
let __ice_render_source_location = ::ui_lang_runtime::testing::Location::new("components/shell.ice", 904, 1, "rendered view node");
#[cfg(test)]
let __ice_render_source = ::ui_lang_runtime::testing::push_render_source(__ice_render_source_location);
let __ice_rendered: __IceElement<'_, __DucktapeMessage> = { let mut __children: ::std::vec::Vec<__IceElement<'_, __DucktapeMessage>> = ::std::vec::Vec::new(); __children.push({
#[cfg(test)]
let __ice_render_source_location = ::ui_lang_runtime::testing::Location::new("components/shell.ice", 907, 1, "rendered view node");
#[cfg(test)]
let __ice_render_source = ::ui_lang_runtime::testing::push_render_source(__ice_render_source_location);
let __ice_rendered: __IceElement<'_, __DucktapeMessage> = (|| { let __slot_content: __IceElement<'_, __DucktapeMessage> = {
#[cfg(test)]
let __ice_render_source_location = ::ui_lang_runtime::testing::Location::new("tests/app.ice", 70, 1, "rendered view node");
#[cfg(test)]
let __ice_render_source = ::ui_lang_runtime::testing::push_render_source(__ice_render_source_location);
let __ice_rendered: __IceElement<'_, __DucktapeMessage> = ::iced::widget::space().width(1.0 as f32).height(1.0 as f32).into();
#[cfg(test)]
drop(__ice_render_source);
__ice_rendered
}; __slot_content })();
#[cfg(test)]
drop(__ice_render_source);
__ice_rendered
}); match &(self.shell_tab) { ShellTab::Chat => { __children.push({
#[cfg(test)]
let __ice_render_source_location = ::ui_lang_runtime::testing::Location::new("components/shell.ice", 910, 1, "rendered view node");
#[cfg(test)]
let __ice_render_source = ::ui_lang_runtime::testing::push_render_source(__ice_render_source_location);
let __ice_rendered: __IceElement<'_, __DucktapeMessage> = (|| { let __slot_content: __IceElement<'_, __DucktapeMessage> = {
#[cfg(test)]
let __ice_render_source_location = ::ui_lang_runtime::testing::Location::new("tests/app.ice", 72, 1, "rendered view node");
#[cfg(test)]
let __ice_render_source = ::ui_lang_runtime::testing::push_render_source(__ice_render_source_location);
let __ice_rendered: __IceElement<'_, __DucktapeMessage> = ::iced::widget::space().width(1.0 as f32).height(1.0 as f32).into();
#[cfg(test)]
drop(__ice_render_source);
__ice_rendered
}; __slot_content })();
#[cfg(test)]
drop(__ice_render_source);
__ice_rendered
}); }, ShellTab::Pages => { __children.push({
#[cfg(test)]
let __ice_render_source_location = ::ui_lang_runtime::testing::Location::new("components/shell.ice", 912, 1, "rendered view node");
#[cfg(test)]
let __ice_render_source = ::ui_lang_runtime::testing::push_render_source(__ice_render_source_location);
let __ice_rendered: __IceElement<'_, __DucktapeMessage> = (|| { let __slot_content: __IceElement<'_, __DucktapeMessage> = {
#[cfg(test)]
let __ice_render_source_location = ::ui_lang_runtime::testing::Location::new("tests/app.ice", 74, 1, "rendered view node");
#[cfg(test)]
let __ice_render_source = ::ui_lang_runtime::testing::push_render_source(__ice_render_source_location);
let __ice_rendered: __IceElement<'_, __DucktapeMessage> = ::iced::widget::space().width(1.0 as f32).height(1.0 as f32).into();
#[cfg(test)]
drop(__ice_render_source);
__ice_rendered
}; __slot_content })();
#[cfg(test)]
drop(__ice_render_source);
__ice_rendered
}); }, ShellTab::Forge => { __children.push({
#[cfg(test)]
let __ice_render_source_location = ::ui_lang_runtime::testing::Location::new("components/shell.ice", 914, 1, "rendered view node");
#[cfg(test)]
let __ice_render_source = ::ui_lang_runtime::testing::push_render_source(__ice_render_source_location);
let __ice_rendered: __IceElement<'_, __DucktapeMessage> = (|| { let __slot_content: __IceElement<'_, __DucktapeMessage> = {
#[cfg(test)]
let __ice_render_source_location = ::ui_lang_runtime::testing::Location::new("tests/app.ice", 82, 1, "rendered view node");
#[cfg(test)]
let __ice_render_source = ::ui_lang_runtime::testing::push_render_source(__ice_render_source_location);
let __ice_rendered: __IceElement<'_, __DucktapeMessage> = ::iced::widget::space().width(1.0 as f32).height(1.0 as f32).into();
#[cfg(test)]
drop(__ice_render_source);
__ice_rendered
}; __slot_content })();
#[cfg(test)]
drop(__ice_render_source);
__ice_rendered
}); }, ShellTab::Agents => { __children.push({
#[cfg(test)]
let __ice_render_source_location = ::ui_lang_runtime::testing::Location::new("components/shell.ice", 916, 1, "rendered view node");
#[cfg(test)]
let __ice_render_source = ::ui_lang_runtime::testing::push_render_source(__ice_render_source_location);
let __ice_rendered: __IceElement<'_, __DucktapeMessage> = (|| { let __slot_content: __IceElement<'_, __DucktapeMessage> = {
#[cfg(test)]
let __ice_render_source_location = ::ui_lang_runtime::testing::Location::new("tests/app.ice", 80, 1, "rendered view node");
#[cfg(test)]
let __ice_render_source = ::ui_lang_runtime::testing::push_render_source(__ice_render_source_location);
let __ice_rendered: __IceElement<'_, __DucktapeMessage> = ::iced::widget::space().width(1.0 as f32).height(1.0 as f32).into();
#[cfg(test)]
drop(__ice_render_source);
__ice_rendered
}; __slot_content })();
#[cfg(test)]
drop(__ice_render_source);
__ice_rendered
}); }, ShellTab::Files => { __children.push({
#[cfg(test)]
let __ice_render_source_location = ::ui_lang_runtime::testing::Location::new("components/shell.ice", 918, 1, "rendered view node");
#[cfg(test)]
let __ice_render_source = ::ui_lang_runtime::testing::push_render_source(__ice_render_source_location);
let __ice_rendered: __IceElement<'_, __DucktapeMessage> = (|| { let __slot_content: __IceElement<'_, __DucktapeMessage> = {
#[cfg(test)]
let __ice_render_source_location = ::ui_lang_runtime::testing::Location::new("tests/app.ice", 76, 1, "rendered view node");
#[cfg(test)]
let __ice_render_source = ::ui_lang_runtime::testing::push_render_source(__ice_render_source_location);
let __ice_rendered: __IceElement<'_, __DucktapeMessage> = ::iced::widget::space().width(1.0 as f32).height(1.0 as f32).into();
#[cfg(test)]
drop(__ice_render_source);
__ice_rendered
}; __slot_content })();
#[cfg(test)]
drop(__ice_render_source);
__ice_rendered
}); }, ShellTab::Explorer => { __children.push({
#[cfg(test)]
let __ice_render_source_location = ::ui_lang_runtime::testing::Location::new("components/shell.ice", 920, 1, "rendered view node");
#[cfg(test)]
let __ice_render_source = ::ui_lang_runtime::testing::push_render_source(__ice_render_source_location);
let __ice_rendered: __IceElement<'_, __DucktapeMessage> = (|| { let __slot_content: __IceElement<'_, __DucktapeMessage> = {
#[cfg(test)]
let __ice_render_source_location = ::ui_lang_runtime::testing::Location::new("tests/app.ice", 90, 1, "rendered view node");
#[cfg(test)]
let __ice_render_source = ::ui_lang_runtime::testing::push_render_source(__ice_render_source_location);
let __ice_rendered: __IceElement<'_, __DucktapeMessage> = ::iced::widget::space().width(1.0 as f32).height(1.0 as f32).into();
#[cfg(test)]
drop(__ice_render_source);
__ice_rendered
}; __slot_content })();
#[cfg(test)]
drop(__ice_render_source);
__ice_rendered
}); }, ShellTab::Node => { __children.push({
#[cfg(test)]
let __ice_render_source_location = ::ui_lang_runtime::testing::Location::new("components/shell.ice", 922, 1, "rendered view node");
#[cfg(test)]
let __ice_render_source = ::ui_lang_runtime::testing::push_render_source(__ice_render_source_location);
let __ice_rendered: __IceElement<'_, __DucktapeMessage> = (|| { let __slot_content: __IceElement<'_, __DucktapeMessage> = {
#[cfg(test)]
let __ice_render_source_location = ::ui_lang_runtime::testing::Location::new("tests/app.ice", 86, 1, "rendered view node");
#[cfg(test)]
let __ice_render_source = ::ui_lang_runtime::testing::push_render_source(__ice_render_source_location);
let __ice_rendered: __IceElement<'_, __DucktapeMessage> = ::iced::widget::space().width(1.0 as f32).height(1.0 as f32).into();
#[cfg(test)]
drop(__ice_render_source);
__ice_rendered
}; __slot_content })();
#[cfg(test)]
drop(__ice_render_source);
__ice_rendered
}); }, ShellTab::Members => { __children.push({
#[cfg(test)]
let __ice_render_source_location = ::ui_lang_runtime::testing::Location::new("components/shell.ice", 924, 1, "rendered view node");
#[cfg(test)]
let __ice_render_source = ::ui_lang_runtime::testing::push_render_source(__ice_render_source_location);
let __ice_rendered: __IceElement<'_, __DucktapeMessage> = (|| { let __slot_content: __IceElement<'_, __DucktapeMessage> = {
#[cfg(test)]
let __ice_render_source_location = ::ui_lang_runtime::testing::Location::new("tests/app.ice", 78, 1, "rendered view node");
#[cfg(test)]
let __ice_render_source = ::ui_lang_runtime::testing::push_render_source(__ice_render_source_location);
let __ice_rendered: __IceElement<'_, __DucktapeMessage> = ::iced::widget::space().width(1.0 as f32).height(1.0 as f32).into();
#[cfg(test)]
drop(__ice_render_source);
__ice_rendered
}; __slot_content })();
#[cfg(test)]
drop(__ice_render_source);
__ice_rendered
}); }, ShellTab::Governance => { __children.push({
#[cfg(test)]
let __ice_render_source_location = ::ui_lang_runtime::testing::Location::new("components/shell.ice", 926, 1, "rendered view node");
#[cfg(test)]
let __ice_render_source = ::ui_lang_runtime::testing::push_render_source(__ice_render_source_location);
let __ice_rendered: __IceElement<'_, __DucktapeMessage> = (|| { let __slot_content: __IceElement<'_, __DucktapeMessage> = {
#[cfg(test)]
let __ice_render_source_location = ::ui_lang_runtime::testing::Location::new("tests/app.ice", 84, 1, "rendered view node");
#[cfg(test)]
let __ice_render_source = ::ui_lang_runtime::testing::push_render_source(__ice_render_source_location);
let __ice_rendered: __IceElement<'_, __DucktapeMessage> = ::iced::widget::space().width(1.0 as f32).height(1.0 as f32).into();
#[cfg(test)]
drop(__ice_render_source);
__ice_rendered
}; __slot_content })();
#[cfg(test)]
drop(__ice_render_source);
__ice_rendered
}); }, ShellTab::Settings => { __children.push({
#[cfg(test)]
let __ice_render_source_location = ::ui_lang_runtime::testing::Location::new("components/shell.ice", 928, 1, "rendered view node");
#[cfg(test)]
let __ice_render_source = ::ui_lang_runtime::testing::push_render_source(__ice_render_source_location);
let __ice_rendered: __IceElement<'_, __DucktapeMessage> = (|| { let __slot_content: __IceElement<'_, __DucktapeMessage> = {
#[cfg(test)]
let __ice_render_source_location = ::ui_lang_runtime::testing::Location::new("tests/app.ice", 88, 1, "rendered view node");
#[cfg(test)]
let __ice_render_source = ::ui_lang_runtime::testing::push_render_source(__ice_render_source_location);
let __ice_rendered: __IceElement<'_, __DucktapeMessage> = ::iced::widget::space().width(1.0 as f32).height(1.0 as f32).into();
#[cfg(test)]
drop(__ice_render_source);
__ice_rendered
}; __slot_content })();
#[cfg(test)]
drop(__ice_render_source);
__ice_rendered
}); }, } let __child_count = __children.len(); let __children = __children.into_iter().map(|__child| ::ui_lang_runtime::bounded_fill_element(__child, __child_count, false)).collect::<::std::vec::Vec<_>>(); let __layout = ::iced::widget::column(__children).width(::iced::Fill).height(::iced::Fill); let __content = ::iced::widget::container(__layout); let __layout_content: __IceElement<'_, __DucktapeMessage> = __content.into(); let __a11y_key = format!("{}/@layout:904", __ice_use_scope); ::ui_lang_runtime::accessible(__layout_content, ::ui_lang_runtime::StableId::new(&__a11y_key), ::ui_lang_runtime::Role::GenericContainer).logical_id_maybe(::core::cfg!(test).then_some(__a11y_key)).into() };
#[cfg(test)]
drop(__ice_render_source);
__ice_rendered
}; let __container = {  ::iced::widget::container(__container_content).id(::iced::widget::Id::from(__ice_node_scope.clone())).width(::iced::Fill).height(::iced::Fill).clip(true).style(move |__theme| { let mut __style = ::iced::widget::container::Style::default(); __style.background = ::std::option::Option::Some(::iced::Background::Color(__ice_palette.colors[2])); __style }) }; ::ui_lang_runtime::accessible(__container, ::ui_lang_runtime::StableId::new(&__a11y_key), ::ui_lang_runtime::Role::GenericContainer).logical_id_maybe(::core::cfg!(test).then_some(__a11y_key)).into() } };
#[cfg(test)]
drop(__ice_render_source);
__ice_rendered
}); let __child_count = __children.len(); let __children = __children.into_iter().map(|__child| ::ui_lang_runtime::bounded_fill_element(__child, __child_count, true)).collect::<::std::vec::Vec<_>>(); let __layout = ::iced::widget::row(__children).width(::iced::Fill).height(::iced::Fill); let __content = ::iced::widget::container(__layout); let __layout_content: __IceElement<'_, __DucktapeMessage> = __content.into(); let __a11y_key = format!("{}/@layout:883", __ice_use_scope); ::ui_lang_runtime::accessible(__layout_content, ::ui_lang_runtime::StableId::new(&__a11y_key), ::ui_lang_runtime::Role::GenericContainer).logical_id_maybe(::core::cfg!(test).then_some(__a11y_key)).into() };
#[cfg(test)]
drop(__ice_render_source);
__ice_rendered
}); let __child_count = __children.len(); let __children = __children.into_iter().map(|__child| ::ui_lang_runtime::bounded_fill_element(__child, __child_count, false)).collect::<::std::vec::Vec<_>>(); let __layout = ::iced::widget::column(__children).width(::iced::Fill).height(::iced::Fill); let __content = ::iced::widget::container(__layout); let __layout_content: __IceElement<'_, __DucktapeMessage> = __content.into(); let __a11y_key = format!("{}/@layout:863", __ice_use_scope); ::ui_lang_runtime::accessible(__layout_content, ::ui_lang_runtime::StableId::new(&__a11y_key), ::ui_lang_runtime::Role::GenericContainer).logical_id_maybe(::core::cfg!(test).then_some(__a11y_key)).into() };
#[cfg(test)]
drop(__ice_render_source);
__ice_rendered
}); __children.push({
#[cfg(test)]
let __ice_render_source_location = ::ui_lang_runtime::testing::Location::new("components/shell.ice", 930, 1, "rendered view node");
#[cfg(test)]
let __ice_render_source = ::ui_lang_runtime::testing::push_render_source(__ice_render_source_location);
let __ice_rendered: __IceElement<'_, __DucktapeMessage> = (|| { let __slot_content: __IceElement<'_, __DucktapeMessage> = {
#[cfg(test)]
let __ice_render_source_location = ::ui_lang_runtime::testing::Location::new("tests/app.ice", 92, 1, "rendered view node");
#[cfg(test)]
let __ice_render_source = ::ui_lang_runtime::testing::push_render_source(__ice_render_source_location);
let __ice_rendered: __IceElement<'_, __DucktapeMessage> = { let mut __children: ::std::vec::Vec<__IceElement<'_, __DucktapeMessage>> = ::std::vec::Vec::new(); if self.palette_open { __children.push({
#[cfg(test)]
let __ice_render_source_location = ::ui_lang_runtime::testing::Location::new("tests/app.ice", 94, 1, "rendered view node");
#[cfg(test)]
let __ice_render_source = ::ui_lang_runtime::testing::push_render_source(__ice_render_source_location);
let __ice_rendered: __IceElement<'_, __DucktapeMessage> = { let __ice_node_scope = format!("{}/palette-input", __ice_use_scope); { let __a11y_key = __ice_node_scope.clone(); let __a11y_id = ::ui_lang_runtime::StableId::new(&__a11y_key); let __disabled = false; let __secure = false; let __role = if __secure { ::ui_lang_runtime::Role::PasswordInput } else { ::ui_lang_runtime::Role::TextInput }; let __input = ::ui_lang_runtime::accessible(::iced::widget::text_input("Search messages and pages… (Esc closes)", &self.palette_draft).id(::iced::widget::Id::from(__a11y_key.clone())).padding(::iced::Padding { top: 11.0, right: 13.0, bottom: 11.0, left: 13.0 }).width(::iced::Fill).secure(__secure).width(540.0 as f32).on_input_maybe(if __disabled { None } else { Some(__DucktapeMessage::__BindPaletteDraft as fn(::std::string::String) -> __DucktapeMessage) }).style(move |__theme, __status| { let mut __style = ::iced::widget::text_input::default(__theme, __status); __style.background = __ice_palette.colors[3].into(); __style.border.color = __ice_palette.colors[39]; __style.border.width = 1.0; __style.border.radius = 10.0.into(); if matches!(__status, ::iced::widget::text_input::Status::Focused { .. }) { __style.border.color = __ice_palette.colors[42]; } __style }), __a11y_id, __role).logical_id_maybe(::core::cfg!(test).then_some(&*__a11y_key)).focus_id(::iced::widget::Id::from(__a11y_key)).label("Search everything".to_owned()).value_maybe((!__secure).then(|| (self.palette_draft).to_owned())).disabled(__disabled); __input.into() } };
#[cfg(test)]
drop(__ice_render_source);
__ice_rendered
}); } let __layout = ::ui_lang_runtime::zstack(__children).width(::iced::Fill).height(::iced::Fill); let __content = ::iced::widget::container(__layout); let __layout_content: __IceElement<'_, __DucktapeMessage> = __content.into(); let __a11y_key = format!("{}/@layout:92", __ice_use_scope); ::ui_lang_runtime::accessible(__layout_content, ::ui_lang_runtime::StableId::new(&__a11y_key), ::ui_lang_runtime::Role::GenericContainer).logical_id_maybe(::core::cfg!(test).then_some(__a11y_key)).into() };
#[cfg(test)]
drop(__ice_render_source);
__ice_rendered
}; __slot_content })();
#[cfg(test)]
drop(__ice_render_source);
__ice_rendered
}); __children.push({
#[cfg(test)]
let __ice_render_source_location = ::ui_lang_runtime::testing::Location::new("components/shell.ice", 931, 1, "rendered view node");
#[cfg(test)]
let __ice_render_source = ::ui_lang_runtime::testing::push_render_source(__ice_render_source_location);
let __ice_rendered: __IceElement<'_, __DucktapeMessage> = (|| { let __slot_content: __IceElement<'_, __DucktapeMessage> = {
#[cfg(test)]
let __ice_render_source_location = ::ui_lang_runtime::testing::Location::new("tests/app.ice", 101, 1, "rendered view node");
#[cfg(test)]
let __ice_render_source = ::ui_lang_runtime::testing::push_render_source(__ice_render_source_location);
let __ice_rendered: __IceElement<'_, __DucktapeMessage> = ::iced::widget::space().width(1.0 as f32).height(1.0 as f32).into();
#[cfg(test)]
drop(__ice_render_source);
__ice_rendered
}; __slot_content })();
#[cfg(test)]
drop(__ice_render_source);
__ice_rendered
}); let __layout = ::ui_lang_runtime::zstack(__children).width(::iced::Fill).height(::iced::Fill); let __content = ::iced::widget::container(__layout); let __layout_content: __IceElement<'_, __DucktapeMessage> = __content.into(); let __a11y_key = format!("{}/@layout:862", __ice_use_scope); ::ui_lang_runtime::accessible(__layout_content, ::ui_lang_runtime::StableId::new(&__a11y_key), ::ui_lang_runtime::Role::GenericContainer).logical_id_maybe(::core::cfg!(test).then_some(__a11y_key)).into() };
#[cfg(test)]
drop(__ice_render_source);
__ice_rendered
}; let __container = {  ::iced::widget::container(__container_content).width(::iced::Fill).height(::iced::Fill).clip(true).style(move |__theme| { let mut __style = ::iced::widget::container::Style::default(); __style.background = ::std::option::Option::Some(::iced::Background::Color(__ice_palette.colors[2])); __style.snap = true; __style }) }; ::ui_lang_runtime::accessible(__container, ::ui_lang_runtime::StableId::new(&__a11y_key), ::ui_lang_runtime::Role::GenericContainer).logical_id_maybe(::core::cfg!(test).then_some(__a11y_key)).into() };
#[cfg(test)]
drop(__ice_render_source);
__ice_rendered
}; __component_content }
#[cfg(test)]
pub(super) fn __ice_component_use_124(&self, __ice_palette: __IcePalette, __ice_use_scope: ::std::string::String) -> __IceElement<'_, __DucktapeMessage> { let __component_content: __IceElement<'_, __DucktapeMessage> = {
#[cfg(test)]
let __ice_render_source_location = ::ui_lang_runtime::testing::Location::new("components/shell.ice", 855, 1, "rendered view node");
#[cfg(test)]
let __ice_render_source = ::ui_lang_runtime::testing::push_render_source(__ice_render_source_location);
let __ice_rendered: __IceElement<'_, __DucktapeMessage> = { let __a11y_key = format!("{}/@container:855", __ice_use_scope); let __container_content: __IceElement<'_, __DucktapeMessage> = {
#[cfg(test)]
let __ice_render_source_location = ::ui_lang_runtime::testing::Location::new("components/shell.ice", 862, 1, "rendered view node");
#[cfg(test)]
let __ice_render_source = ::ui_lang_runtime::testing::push_render_source(__ice_render_source_location);
let __ice_rendered: __IceElement<'_, __DucktapeMessage> = { let mut __children: ::std::vec::Vec<__IceElement<'_, __DucktapeMessage>> = ::std::vec::Vec::new(); __children.push({
#[cfg(test)]
let __ice_render_source_location = ::ui_lang_runtime::testing::Location::new("components/shell.ice", 863, 1, "rendered view node");
#[cfg(test)]
let __ice_render_source = ::ui_lang_runtime::testing::push_render_source(__ice_render_source_location);
let __ice_rendered: __IceElement<'_, __DucktapeMessage> = { let mut __children: ::std::vec::Vec<__IceElement<'_, __DucktapeMessage>> = ::std::vec::Vec::new(); __children.push({
#[cfg(test)]
let __ice_render_source_location = ::ui_lang_runtime::testing::Location::new("components/shell.ice", 864, 1, "rendered view node");
#[cfg(test)]
let __ice_render_source = ::ui_lang_runtime::testing::push_render_source(__ice_render_source_location);
let __ice_rendered: __IceElement<'_, __DucktapeMessage> = { let __ice_node_scope = format!("{}/titlebar", __ice_use_scope); ::ui_lang_runtime::grow_stack(|| self.__ice_component_use_106(__ice_palette, __ice_node_scope.clone(), (move || __DucktapeMessage::SwitchNetwork).clone(), (move || __DucktapeMessage::ToggleBell).clone())) };
#[cfg(test)]
drop(__ice_render_source);
__ice_rendered
}); __children.push({
#[cfg(test)]
let __ice_render_source_location = ::ui_lang_runtime::testing::Location::new("components/shell.ice", 883, 1, "rendered view node");
#[cfg(test)]
let __ice_render_source = ::ui_lang_runtime::testing::push_render_source(__ice_render_source_location);
let __ice_rendered: __IceElement<'_, __DucktapeMessage> = { let mut __children: ::std::vec::Vec<__IceElement<'_, __DucktapeMessage>> = ::std::vec::Vec::new(); __children.push({
#[cfg(test)]
let __ice_render_source_location = ::ui_lang_runtime::testing::Location::new("components/shell.ice", 884, 1, "rendered view node");
#[cfg(test)]
let __ice_render_source = ::ui_lang_runtime::testing::push_render_source(__ice_render_source_location);
let __ice_rendered: __IceElement<'_, __DucktapeMessage> = { let __ice_node_scope = format!("{}/rail", __ice_use_scope); ::ui_lang_runtime::grow_stack(|| self.__ice_component_use_117(__ice_palette, __ice_node_scope.clone(), (move |__event_0| __DucktapeMessage::SelectShellTab(__event_0)).clone())) };
#[cfg(test)]
drop(__ice_render_source);
__ice_rendered
}); __children.push({
#[cfg(test)]
let __ice_render_source_location = ::ui_lang_runtime::testing::Location::new("components/shell.ice", 892, 1, "rendered view node");
#[cfg(test)]
let __ice_render_source = ::ui_lang_runtime::testing::push_render_source(__ice_render_source_location);
let __ice_rendered: __IceElement<'_, __DucktapeMessage> = { let __a11y_key = format!("{}/@container:892", __ice_use_scope); let __container_content: __IceElement<'_, __DucktapeMessage> = {
#[cfg(test)]
let __ice_render_source_location = ::ui_lang_runtime::testing::Location::new("components/shell.ice", 897, 1, "rendered view node");
#[cfg(test)]
let __ice_render_source = ::ui_lang_runtime::testing::push_render_source(__ice_render_source_location);
let __ice_rendered: __IceElement<'_, __DucktapeMessage> = ::iced::widget::space().width(1.0 as f32).height(1.0 as f32).into();
#[cfg(test)]
drop(__ice_render_source);
__ice_rendered
}; let __container = {  ::iced::widget::container(__container_content).width(1.0 as f32).height(::iced::Fill).style(move |__theme| { let mut __style = ::iced::widget::container::Style::default(); __style.background = ::std::option::Option::Some(::iced::Background::Color(__ice_palette.colors[60])); __style }) }; ::ui_lang_runtime::accessible(__container, ::ui_lang_runtime::StableId::new(&__a11y_key), ::ui_lang_runtime::Role::GenericContainer).logical_id_maybe(::core::cfg!(test).then_some(__a11y_key)).into() };
#[cfg(test)]
drop(__ice_render_source);
__ice_rendered
}); __children.push({
#[cfg(test)]
let __ice_render_source_location = ::ui_lang_runtime::testing::Location::new("components/shell.ice", 898, 1, "rendered view node");
#[cfg(test)]
let __ice_render_source = ::ui_lang_runtime::testing::push_render_source(__ice_render_source_location);
let __ice_rendered: __IceElement<'_, __DucktapeMessage> = { let __ice_node_scope = format!("{}/content", __ice_use_scope); { let __a11y_key = __ice_node_scope.as_str(); let __container_content: __IceElement<'_, __DucktapeMessage> = {
#[cfg(test)]
let __ice_render_source_location = ::ui_lang_runtime::testing::Location::new("components/shell.ice", 904, 1, "rendered view node");
#[cfg(test)]
let __ice_render_source = ::ui_lang_runtime::testing::push_render_source(__ice_render_source_location);
let __ice_rendered: __IceElement<'_, __DucktapeMessage> = { let mut __children: ::std::vec::Vec<__IceElement<'_, __DucktapeMessage>> = ::std::vec::Vec::new(); __children.push({
#[cfg(test)]
let __ice_render_source_location = ::ui_lang_runtime::testing::Location::new("components/shell.ice", 907, 1, "rendered view node");
#[cfg(test)]
let __ice_render_source = ::ui_lang_runtime::testing::push_render_source(__ice_render_source_location);
let __ice_rendered: __IceElement<'_, __DucktapeMessage> = (|| { let __slot_content: __IceElement<'_, __DucktapeMessage> = {
#[cfg(test)]
let __ice_render_source_location = ::ui_lang_runtime::testing::Location::new("tests/app.ice", 200, 1, "rendered view node");
#[cfg(test)]
let __ice_render_source = ::ui_lang_runtime::testing::push_render_source(__ice_render_source_location);
let __ice_rendered: __IceElement<'_, __DucktapeMessage> = ::iced::widget::space().width(1.0 as f32).height(1.0 as f32).into();
#[cfg(test)]
drop(__ice_render_source);
__ice_rendered
}; __slot_content })();
#[cfg(test)]
drop(__ice_render_source);
__ice_rendered
}); match &(self.shell_tab) { ShellTab::Chat => { __children.push({
#[cfg(test)]
let __ice_render_source_location = ::ui_lang_runtime::testing::Location::new("components/shell.ice", 910, 1, "rendered view node");
#[cfg(test)]
let __ice_render_source = ::ui_lang_runtime::testing::push_render_source(__ice_render_source_location);
let __ice_rendered: __IceElement<'_, __DucktapeMessage> = (|| { let __slot_content: __IceElement<'_, __DucktapeMessage> = {
#[cfg(test)]
let __ice_render_source_location = ::ui_lang_runtime::testing::Location::new("tests/app.ice", 202, 1, "rendered view node");
#[cfg(test)]
let __ice_render_source = ::ui_lang_runtime::testing::push_render_source(__ice_render_source_location);
let __ice_rendered: __IceElement<'_, __DucktapeMessage> = ::iced::widget::space().width(1.0 as f32).height(1.0 as f32).into();
#[cfg(test)]
drop(__ice_render_source);
__ice_rendered
}; __slot_content })();
#[cfg(test)]
drop(__ice_render_source);
__ice_rendered
}); }, ShellTab::Pages => { __children.push({
#[cfg(test)]
let __ice_render_source_location = ::ui_lang_runtime::testing::Location::new("components/shell.ice", 912, 1, "rendered view node");
#[cfg(test)]
let __ice_render_source = ::ui_lang_runtime::testing::push_render_source(__ice_render_source_location);
let __ice_rendered: __IceElement<'_, __DucktapeMessage> = (|| { let __slot_content: __IceElement<'_, __DucktapeMessage> = {
#[cfg(test)]
let __ice_render_source_location = ::ui_lang_runtime::testing::Location::new("tests/app.ice", 204, 1, "rendered view node");
#[cfg(test)]
let __ice_render_source = ::ui_lang_runtime::testing::push_render_source(__ice_render_source_location);
let __ice_rendered: __IceElement<'_, __DucktapeMessage> = ::iced::widget::space().width(1.0 as f32).height(1.0 as f32).into();
#[cfg(test)]
drop(__ice_render_source);
__ice_rendered
}; __slot_content })();
#[cfg(test)]
drop(__ice_render_source);
__ice_rendered
}); }, ShellTab::Forge => { __children.push({
#[cfg(test)]
let __ice_render_source_location = ::ui_lang_runtime::testing::Location::new("components/shell.ice", 914, 1, "rendered view node");
#[cfg(test)]
let __ice_render_source = ::ui_lang_runtime::testing::push_render_source(__ice_render_source_location);
let __ice_rendered: __IceElement<'_, __DucktapeMessage> = (|| { let __slot_content: __IceElement<'_, __DucktapeMessage> = {
#[cfg(test)]
let __ice_render_source_location = ::ui_lang_runtime::testing::Location::new("tests/app.ice", 212, 1, "rendered view node");
#[cfg(test)]
let __ice_render_source = ::ui_lang_runtime::testing::push_render_source(__ice_render_source_location);
let __ice_rendered: __IceElement<'_, __DucktapeMessage> = ::iced::widget::space().width(1.0 as f32).height(1.0 as f32).into();
#[cfg(test)]
drop(__ice_render_source);
__ice_rendered
}; __slot_content })();
#[cfg(test)]
drop(__ice_render_source);
__ice_rendered
}); }, ShellTab::Agents => { __children.push({
#[cfg(test)]
let __ice_render_source_location = ::ui_lang_runtime::testing::Location::new("components/shell.ice", 916, 1, "rendered view node");
#[cfg(test)]
let __ice_render_source = ::ui_lang_runtime::testing::push_render_source(__ice_render_source_location);
let __ice_rendered: __IceElement<'_, __DucktapeMessage> = (|| { let __slot_content: __IceElement<'_, __DucktapeMessage> = {
#[cfg(test)]
let __ice_render_source_location = ::ui_lang_runtime::testing::Location::new("tests/app.ice", 210, 1, "rendered view node");
#[cfg(test)]
let __ice_render_source = ::ui_lang_runtime::testing::push_render_source(__ice_render_source_location);
let __ice_rendered: __IceElement<'_, __DucktapeMessage> = ::iced::widget::space().width(1.0 as f32).height(1.0 as f32).into();
#[cfg(test)]
drop(__ice_render_source);
__ice_rendered
}; __slot_content })();
#[cfg(test)]
drop(__ice_render_source);
__ice_rendered
}); }, ShellTab::Files => { __children.push({
#[cfg(test)]
let __ice_render_source_location = ::ui_lang_runtime::testing::Location::new("components/shell.ice", 918, 1, "rendered view node");
#[cfg(test)]
let __ice_render_source = ::ui_lang_runtime::testing::push_render_source(__ice_render_source_location);
let __ice_rendered: __IceElement<'_, __DucktapeMessage> = (|| { let __slot_content: __IceElement<'_, __DucktapeMessage> = {
#[cfg(test)]
let __ice_render_source_location = ::ui_lang_runtime::testing::Location::new("tests/app.ice", 206, 1, "rendered view node");
#[cfg(test)]
let __ice_render_source = ::ui_lang_runtime::testing::push_render_source(__ice_render_source_location);
let __ice_rendered: __IceElement<'_, __DucktapeMessage> = ::iced::widget::space().width(1.0 as f32).height(1.0 as f32).into();
#[cfg(test)]
drop(__ice_render_source);
__ice_rendered
}; __slot_content })();
#[cfg(test)]
drop(__ice_render_source);
__ice_rendered
}); }, ShellTab::Explorer => { __children.push({
#[cfg(test)]
let __ice_render_source_location = ::ui_lang_runtime::testing::Location::new("components/shell.ice", 920, 1, "rendered view node");
#[cfg(test)]
let __ice_render_source = ::ui_lang_runtime::testing::push_render_source(__ice_render_source_location);
let __ice_rendered: __IceElement<'_, __DucktapeMessage> = (|| { let __slot_content: __IceElement<'_, __DucktapeMessage> = {
#[cfg(test)]
let __ice_render_source_location = ::ui_lang_runtime::testing::Location::new("tests/app.ice", 220, 1, "rendered view node");
#[cfg(test)]
let __ice_render_source = ::ui_lang_runtime::testing::push_render_source(__ice_render_source_location);
let __ice_rendered: __IceElement<'_, __DucktapeMessage> = ::iced::widget::space().width(1.0 as f32).height(1.0 as f32).into();
#[cfg(test)]
drop(__ice_render_source);
__ice_rendered
}; __slot_content })();
#[cfg(test)]
drop(__ice_render_source);
__ice_rendered
}); }, ShellTab::Node => { __children.push({
#[cfg(test)]
let __ice_render_source_location = ::ui_lang_runtime::testing::Location::new("components/shell.ice", 922, 1, "rendered view node");
#[cfg(test)]
let __ice_render_source = ::ui_lang_runtime::testing::push_render_source(__ice_render_source_location);
let __ice_rendered: __IceElement<'_, __DucktapeMessage> = (|| { let __slot_content: __IceElement<'_, __DucktapeMessage> = {
#[cfg(test)]
let __ice_render_source_location = ::ui_lang_runtime::testing::Location::new("tests/app.ice", 216, 1, "rendered view node");
#[cfg(test)]
let __ice_render_source = ::ui_lang_runtime::testing::push_render_source(__ice_render_source_location);
let __ice_rendered: __IceElement<'_, __DucktapeMessage> = ::iced::widget::space().width(1.0 as f32).height(1.0 as f32).into();
#[cfg(test)]
drop(__ice_render_source);
__ice_rendered
}; __slot_content })();
#[cfg(test)]
drop(__ice_render_source);
__ice_rendered
}); }, ShellTab::Members => { __children.push({
#[cfg(test)]
let __ice_render_source_location = ::ui_lang_runtime::testing::Location::new("components/shell.ice", 924, 1, "rendered view node");
#[cfg(test)]
let __ice_render_source = ::ui_lang_runtime::testing::push_render_source(__ice_render_source_location);
let __ice_rendered: __IceElement<'_, __DucktapeMessage> = (|| { let __slot_content: __IceElement<'_, __DucktapeMessage> = {
#[cfg(test)]
let __ice_render_source_location = ::ui_lang_runtime::testing::Location::new("tests/app.ice", 208, 1, "rendered view node");
#[cfg(test)]
let __ice_render_source = ::ui_lang_runtime::testing::push_render_source(__ice_render_source_location);
let __ice_rendered: __IceElement<'_, __DucktapeMessage> = ::iced::widget::space().width(1.0 as f32).height(1.0 as f32).into();
#[cfg(test)]
drop(__ice_render_source);
__ice_rendered
}; __slot_content })();
#[cfg(test)]
drop(__ice_render_source);
__ice_rendered
}); }, ShellTab::Governance => { __children.push({
#[cfg(test)]
let __ice_render_source_location = ::ui_lang_runtime::testing::Location::new("components/shell.ice", 926, 1, "rendered view node");
#[cfg(test)]
let __ice_render_source = ::ui_lang_runtime::testing::push_render_source(__ice_render_source_location);
let __ice_rendered: __IceElement<'_, __DucktapeMessage> = (|| { let __slot_content: __IceElement<'_, __DucktapeMessage> = {
#[cfg(test)]
let __ice_render_source_location = ::ui_lang_runtime::testing::Location::new("tests/app.ice", 214, 1, "rendered view node");
#[cfg(test)]
let __ice_render_source = ::ui_lang_runtime::testing::push_render_source(__ice_render_source_location);
let __ice_rendered: __IceElement<'_, __DucktapeMessage> = ::iced::widget::space().width(1.0 as f32).height(1.0 as f32).into();
#[cfg(test)]
drop(__ice_render_source);
__ice_rendered
}; __slot_content })();
#[cfg(test)]
drop(__ice_render_source);
__ice_rendered
}); }, ShellTab::Settings => { __children.push({
#[cfg(test)]
let __ice_render_source_location = ::ui_lang_runtime::testing::Location::new("components/shell.ice", 928, 1, "rendered view node");
#[cfg(test)]
let __ice_render_source = ::ui_lang_runtime::testing::push_render_source(__ice_render_source_location);
let __ice_rendered: __IceElement<'_, __DucktapeMessage> = (|| { let __slot_content: __IceElement<'_, __DucktapeMessage> = {
#[cfg(test)]
let __ice_render_source_location = ::ui_lang_runtime::testing::Location::new("tests/app.ice", 218, 1, "rendered view node");
#[cfg(test)]
let __ice_render_source = ::ui_lang_runtime::testing::push_render_source(__ice_render_source_location);
let __ice_rendered: __IceElement<'_, __DucktapeMessage> = ::iced::widget::space().width(1.0 as f32).height(1.0 as f32).into();
#[cfg(test)]
drop(__ice_render_source);
__ice_rendered
}; __slot_content })();
#[cfg(test)]
drop(__ice_render_source);
__ice_rendered
}); }, } let __child_count = __children.len(); let __children = __children.into_iter().map(|__child| ::ui_lang_runtime::bounded_fill_element(__child, __child_count, false)).collect::<::std::vec::Vec<_>>(); let __layout = ::iced::widget::column(__children).width(::iced::Fill).height(::iced::Fill); let __content = ::iced::widget::container(__layout); let __layout_content: __IceElement<'_, __DucktapeMessage> = __content.into(); let __a11y_key = format!("{}/@layout:904", __ice_use_scope); ::ui_lang_runtime::accessible(__layout_content, ::ui_lang_runtime::StableId::new(&__a11y_key), ::ui_lang_runtime::Role::GenericContainer).logical_id_maybe(::core::cfg!(test).then_some(__a11y_key)).into() };
#[cfg(test)]
drop(__ice_render_source);
__ice_rendered
}; let __container = {  ::iced::widget::container(__container_content).id(::iced::widget::Id::from(__ice_node_scope.clone())).width(::iced::Fill).height(::iced::Fill).clip(true).style(move |__theme| { let mut __style = ::iced::widget::container::Style::default(); __style.background = ::std::option::Option::Some(::iced::Background::Color(__ice_palette.colors[2])); __style }) }; ::ui_lang_runtime::accessible(__container, ::ui_lang_runtime::StableId::new(&__a11y_key), ::ui_lang_runtime::Role::GenericContainer).logical_id_maybe(::core::cfg!(test).then_some(__a11y_key)).into() } };
#[cfg(test)]
drop(__ice_render_source);
__ice_rendered
}); let __child_count = __children.len(); let __children = __children.into_iter().map(|__child| ::ui_lang_runtime::bounded_fill_element(__child, __child_count, true)).collect::<::std::vec::Vec<_>>(); let __layout = ::iced::widget::row(__children).width(::iced::Fill).height(::iced::Fill); let __content = ::iced::widget::container(__layout); let __layout_content: __IceElement<'_, __DucktapeMessage> = __content.into(); let __a11y_key = format!("{}/@layout:883", __ice_use_scope); ::ui_lang_runtime::accessible(__layout_content, ::ui_lang_runtime::StableId::new(&__a11y_key), ::ui_lang_runtime::Role::GenericContainer).logical_id_maybe(::core::cfg!(test).then_some(__a11y_key)).into() };
#[cfg(test)]
drop(__ice_render_source);
__ice_rendered
}); let __child_count = __children.len(); let __children = __children.into_iter().map(|__child| ::ui_lang_runtime::bounded_fill_element(__child, __child_count, false)).collect::<::std::vec::Vec<_>>(); let __layout = ::iced::widget::column(__children).width(::iced::Fill).height(::iced::Fill); let __content = ::iced::widget::container(__layout); let __layout_content: __IceElement<'_, __DucktapeMessage> = __content.into(); let __a11y_key = format!("{}/@layout:863", __ice_use_scope); ::ui_lang_runtime::accessible(__layout_content, ::ui_lang_runtime::StableId::new(&__a11y_key), ::ui_lang_runtime::Role::GenericContainer).logical_id_maybe(::core::cfg!(test).then_some(__a11y_key)).into() };
#[cfg(test)]
drop(__ice_render_source);
__ice_rendered
}); __children.push({
#[cfg(test)]
let __ice_render_source_location = ::ui_lang_runtime::testing::Location::new("components/shell.ice", 930, 1, "rendered view node");
#[cfg(test)]
let __ice_render_source = ::ui_lang_runtime::testing::push_render_source(__ice_render_source_location);
let __ice_rendered: __IceElement<'_, __DucktapeMessage> = (|| { let __slot_content: __IceElement<'_, __DucktapeMessage> = {
#[cfg(test)]
let __ice_render_source_location = ::ui_lang_runtime::testing::Location::new("tests/app.ice", 222, 1, "rendered view node");
#[cfg(test)]
let __ice_render_source = ::ui_lang_runtime::testing::push_render_source(__ice_render_source_location);
let __ice_rendered: __IceElement<'_, __DucktapeMessage> = ::iced::widget::space().width(1.0 as f32).height(1.0 as f32).into();
#[cfg(test)]
drop(__ice_render_source);
__ice_rendered
}; __slot_content })();
#[cfg(test)]
drop(__ice_render_source);
__ice_rendered
}); __children.push({
#[cfg(test)]
let __ice_render_source_location = ::ui_lang_runtime::testing::Location::new("components/shell.ice", 931, 1, "rendered view node");
#[cfg(test)]
let __ice_render_source = ::ui_lang_runtime::testing::push_render_source(__ice_render_source_location);
let __ice_rendered: __IceElement<'_, __DucktapeMessage> = (|| { let __slot_content: __IceElement<'_, __DucktapeMessage> = {
#[cfg(test)]
let __ice_render_source_location = ::ui_lang_runtime::testing::Location::new("tests/app.ice", 224, 1, "rendered view node");
#[cfg(test)]
let __ice_render_source = ::ui_lang_runtime::testing::push_render_source(__ice_render_source_location);
let __ice_rendered: __IceElement<'_, __DucktapeMessage> = ::iced::widget::space().width(1.0 as f32).height(1.0 as f32).into();
#[cfg(test)]
drop(__ice_render_source);
__ice_rendered
}; __slot_content })();
#[cfg(test)]
drop(__ice_render_source);
__ice_rendered
}); let __layout = ::ui_lang_runtime::zstack(__children).width(::iced::Fill).height(::iced::Fill); let __content = ::iced::widget::container(__layout); let __layout_content: __IceElement<'_, __DucktapeMessage> = __content.into(); let __a11y_key = format!("{}/@layout:862", __ice_use_scope); ::ui_lang_runtime::accessible(__layout_content, ::ui_lang_runtime::StableId::new(&__a11y_key), ::ui_lang_runtime::Role::GenericContainer).logical_id_maybe(::core::cfg!(test).then_some(__a11y_key)).into() };
#[cfg(test)]
drop(__ice_render_source);
__ice_rendered
}; let __container = {  ::iced::widget::container(__container_content).width(::iced::Fill).height(::iced::Fill).clip(true).style(move |__theme| { let mut __style = ::iced::widget::container::Style::default(); __style.background = ::std::option::Option::Some(::iced::Background::Color(__ice_palette.colors[2])); __style.snap = true; __style }) }; ::ui_lang_runtime::accessible(__container, ::ui_lang_runtime::StableId::new(&__a11y_key), ::ui_lang_runtime::Role::GenericContainer).logical_id_maybe(::core::cfg!(test).then_some(__a11y_key)).into() };
#[cfg(test)]
drop(__ice_render_source);
__ice_rendered
}; __component_content }
}
}
