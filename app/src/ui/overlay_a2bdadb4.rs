#[allow(warnings, clippy::all)]
mod __ice_group_overlay_a2bdadb4 {
use super::*;
impl super::Ducktape {
pub(super) fn __ice_component_use_85(&self, __ice_palette: __IcePalette, __ice_use_scope: ::std::string::String) -> __IceElement<'_, __DucktapeMessage> { let __component_content: __IceElement<'_, __DucktapeMessage> = {
#[cfg(test)]
let __ice_render_source_location = ::ui_lang_runtime::testing::Location::new("components/overlay.ice", 253, 1, "rendered view node");
#[cfg(test)]
let __ice_render_source = ::ui_lang_runtime::testing::push_render_source(__ice_render_source_location);
let __ice_rendered: __IceElement<'_, __DucktapeMessage> = { let __ice_node_scope = format!("{}/root", __ice_use_scope); { let mut __children: ::std::vec::Vec<__IceElement<'_, __DucktapeMessage>> = ::std::vec::Vec::new(); __children.push({
#[cfg(test)]
let __ice_render_source_location = ::ui_lang_runtime::testing::Location::new("components/overlay.ice", 254, 1, "rendered view node");
#[cfg(test)]
let __ice_render_source = ::ui_lang_runtime::testing::push_render_source(__ice_render_source_location);
let __ice_rendered: __IceElement<'_, __DucktapeMessage> = { let __a11y_key = format!("{}/@text:254", __ice_use_scope); let __text_value = ("CHANNEL NAME").to_string(); let __text = ::iced::widget::text(__text_value.clone()).size(((9.0) as f32).max(f32::EPSILON).min(f32::MAX)).wrapping(::iced::widget::text::Wrapping::None).font(::iced::Font { family: ::iced::font::Family::Name("Geist Mono"), weight: ::iced::font::Weight::Semibold, stretch: ::iced::font::Stretch::Normal, style: ::iced::font::Style::Normal }).color(__ice_palette.colors[73]); let __text = ::ui_lang_runtime::selectable_text(__text); ::ui_lang_runtime::accessible(__text, ::ui_lang_runtime::StableId::new(&__a11y_key), ::ui_lang_runtime::Role::Label).logical_id_maybe(::core::cfg!(test).then_some(__a11y_key)).value(__text_value).into() };
#[cfg(test)]
drop(__ice_render_source);
__ice_rendered
}); if ("" != "") { __children.push({
#[cfg(test)]
let __ice_render_source_location = ::ui_lang_runtime::testing::Location::new("components/overlay.ice", 261, 1, "rendered view node");
#[cfg(test)]
let __ice_render_source = ::ui_lang_runtime::testing::push_render_source(__ice_render_source_location);
let __ice_rendered: __IceElement<'_, __DucktapeMessage> = { let __a11y_key = format!("{}/@text:261", __ice_use_scope); let __text_value = ("").to_string(); let __text = ::iced::widget::text(__text_value.clone()).size(((9.0) as f32).max(f32::EPSILON).min(f32::MAX)).wrapping(::iced::widget::text::Wrapping::None).font(::iced::Font { family: ::iced::font::Family::Name("Geist Mono"), weight: ::iced::font::Weight::Semibold, stretch: ::iced::font::Stretch::Normal, style: ::iced::font::Style::Normal }).color(__ice_palette.colors[73]); let __text = ::ui_lang_runtime::selectable_text(__text); ::ui_lang_runtime::accessible(__text, ::ui_lang_runtime::StableId::new(&__a11y_key), ::ui_lang_runtime::Role::Label).logical_id_maybe(::core::cfg!(test).then_some(__a11y_key)).value(__text_value).into() };
#[cfg(test)]
drop(__ice_render_source);
__ice_rendered
}); } let __child_count = __children.len(); let __children = __children.into_iter().map(|__child| ::ui_lang_runtime::bounded_fill_element(__child, __child_count, true)).collect::<::std::vec::Vec<_>>(); let __layout = ::iced::widget::row(__children).spacing(::ui_lang_runtime::bounded_spacing(8.0, __child_count)).align_y(::iced::alignment::Vertical::Center); let __content = ::iced::widget::container(__layout); let __layout_content: __IceElement<'_, __DucktapeMessage> = __content.into(); let __a11y_key = __ice_node_scope.as_str(); ::ui_lang_runtime::accessible(__layout_content, ::ui_lang_runtime::StableId::new(&__a11y_key), ::ui_lang_runtime::Role::GenericContainer).logical_id_maybe(::core::cfg!(test).then_some(__a11y_key)).into() } };
#[cfg(test)]
drop(__ice_render_source);
__ice_rendered
}; __component_content }
pub(super) fn __ice_component_use_86(&self, __ice_palette: __IcePalette, __ice_use_scope: ::std::string::String) -> __IceElement<'_, __DucktapeMessage> { let __component_content: __IceElement<'_, __DucktapeMessage> = {
#[cfg(test)]
let __ice_render_source_location = ::ui_lang_runtime::testing::Location::new("components/overlay.ice", 253, 1, "rendered view node");
#[cfg(test)]
let __ice_render_source = ::ui_lang_runtime::testing::push_render_source(__ice_render_source_location);
let __ice_rendered: __IceElement<'_, __DucktapeMessage> = { let __ice_node_scope = format!("{}/root", __ice_use_scope); { let mut __children: ::std::vec::Vec<__IceElement<'_, __DucktapeMessage>> = ::std::vec::Vec::new(); __children.push({
#[cfg(test)]
let __ice_render_source_location = ::ui_lang_runtime::testing::Location::new("components/overlay.ice", 254, 1, "rendered view node");
#[cfg(test)]
let __ice_render_source = ::ui_lang_runtime::testing::push_render_source(__ice_render_source_location);
let __ice_rendered: __IceElement<'_, __DucktapeMessage> = { let __a11y_key = format!("{}/@text:254", __ice_use_scope); let __text_value = ("POSTING").to_string(); let __text = ::iced::widget::text(__text_value.clone()).size(((9.0) as f32).max(f32::EPSILON).min(f32::MAX)).wrapping(::iced::widget::text::Wrapping::None).font(::iced::Font { family: ::iced::font::Family::Name("Geist Mono"), weight: ::iced::font::Weight::Semibold, stretch: ::iced::font::Stretch::Normal, style: ::iced::font::Style::Normal }).color(__ice_palette.colors[73]); let __text = ::ui_lang_runtime::selectable_text(__text); ::ui_lang_runtime::accessible(__text, ::ui_lang_runtime::StableId::new(&__a11y_key), ::ui_lang_runtime::Role::Label).logical_id_maybe(::core::cfg!(test).then_some(__a11y_key)).value(__text_value).into() };
#[cfg(test)]
drop(__ice_render_source);
__ice_rendered
}); if ("" != "") { __children.push({
#[cfg(test)]
let __ice_render_source_location = ::ui_lang_runtime::testing::Location::new("components/overlay.ice", 261, 1, "rendered view node");
#[cfg(test)]
let __ice_render_source = ::ui_lang_runtime::testing::push_render_source(__ice_render_source_location);
let __ice_rendered: __IceElement<'_, __DucktapeMessage> = { let __a11y_key = format!("{}/@text:261", __ice_use_scope); let __text_value = ("").to_string(); let __text = ::iced::widget::text(__text_value.clone()).size(((9.0) as f32).max(f32::EPSILON).min(f32::MAX)).wrapping(::iced::widget::text::Wrapping::None).font(::iced::Font { family: ::iced::font::Family::Name("Geist Mono"), weight: ::iced::font::Weight::Semibold, stretch: ::iced::font::Stretch::Normal, style: ::iced::font::Style::Normal }).color(__ice_palette.colors[73]); let __text = ::ui_lang_runtime::selectable_text(__text); ::ui_lang_runtime::accessible(__text, ::ui_lang_runtime::StableId::new(&__a11y_key), ::ui_lang_runtime::Role::Label).logical_id_maybe(::core::cfg!(test).then_some(__a11y_key)).value(__text_value).into() };
#[cfg(test)]
drop(__ice_render_source);
__ice_rendered
}); } let __child_count = __children.len(); let __children = __children.into_iter().map(|__child| ::ui_lang_runtime::bounded_fill_element(__child, __child_count, true)).collect::<::std::vec::Vec<_>>(); let __layout = ::iced::widget::row(__children).spacing(::ui_lang_runtime::bounded_spacing(8.0, __child_count)).align_y(::iced::alignment::Vertical::Center); let __content = ::iced::widget::container(__layout); let __layout_content: __IceElement<'_, __DucktapeMessage> = __content.into(); let __a11y_key = __ice_node_scope.as_str(); ::ui_lang_runtime::accessible(__layout_content, ::ui_lang_runtime::StableId::new(&__a11y_key), ::ui_lang_runtime::Role::GenericContainer).logical_id_maybe(::core::cfg!(test).then_some(__a11y_key)).into() } };
#[cfg(test)]
drop(__ice_render_source);
__ice_rendered
}; __component_content }
pub(super) fn __ice_component_use_87(&self, __ice_palette: __IcePalette, __ice_use_scope: ::std::string::String, __ice_cb_0: impl Fn() -> __DucktapeMessage + Clone + 'static, __ice_cb_1: impl Fn() -> __DucktapeMessage + Clone + 'static, __ice_cb_2: impl Fn() -> __DucktapeMessage + Clone + 'static, __ice_cb_3: impl Fn(::std::string::String, i64) -> __DucktapeMessage + Clone + 'static, __ice_cb_4: impl Fn(::std::string::String, ::std::string::String) -> __DucktapeMessage + Clone + 'static, __ice_cb_5: impl Fn(::std::string::String) -> __DucktapeMessage + Clone + 'static, __ice_cb_6: impl Fn() -> __DucktapeMessage + Clone + 'static, __ice_cb_7: impl Fn() -> __DucktapeMessage + Clone + 'static) -> __IceElement<'_, __DucktapeMessage> { let __component_content: __IceElement<'_, __DucktapeMessage> = {
#[cfg(test)]
let __ice_render_source_location = ::ui_lang_runtime::testing::Location::new("components/overlay.ice", 87, 1, "rendered view node");
#[cfg(test)]
let __ice_render_source = ::ui_lang_runtime::testing::push_render_source(__ice_render_source_location);
let __ice_rendered: __IceElement<'_, __DucktapeMessage> = { let __ice_node_scope = format!("{}/root", __ice_use_scope); { let __a11y_key = __ice_node_scope.as_str(); let __container_content: __IceElement<'_, __DucktapeMessage> = {
#[cfg(test)]
let __ice_render_source_location = ::ui_lang_runtime::testing::Location::new("components/overlay.ice", 97, 1, "rendered view node");
#[cfg(test)]
let __ice_render_source = ::ui_lang_runtime::testing::push_render_source(__ice_render_source_location);
let __ice_rendered: __IceElement<'_, __DucktapeMessage> = { let mut __children: ::std::vec::Vec<__IceElement<'_, __DucktapeMessage>> = ::std::vec::Vec::new(); __children.push({
#[cfg(test)]
let __ice_render_source_location = ::ui_lang_runtime::testing::Location::new("components/overlay.ice", 105, 1, "rendered view node");
#[cfg(test)]
let __ice_render_source = ::ui_lang_runtime::testing::push_render_source(__ice_render_source_location);
let __ice_rendered: __IceElement<'_, __DucktapeMessage> = { let mut __children: ::std::vec::Vec<__IceElement<'_, __DucktapeMessage>> = ::std::vec::Vec::new(); __children.push({
#[cfg(test)]
let __ice_render_source_location = ::ui_lang_runtime::testing::Location::new("components/overlay.ice", 110, 1, "rendered view node");
#[cfg(test)]
let __ice_render_source = ::ui_lang_runtime::testing::push_render_source(__ice_render_source_location);
let __ice_rendered: __IceElement<'_, __DucktapeMessage> = { let __a11y_key = format!("{}/@text:110", __ice_use_scope); let __text_value = ("Create a channel").to_string(); let __text = ::iced::widget::text(__text_value.clone()).size(((16.0) as f32).max(f32::EPSILON).min(f32::MAX)).width(::iced::Fill).wrapping(::iced::widget::text::Wrapping::None).font(::iced::Font { family: ::iced::font::Family::Name("Geist"), weight: ::iced::font::Weight::Semibold, stretch: ::iced::font::Stretch::Normal, style: ::iced::font::Style::Normal }).color(__ice_palette.colors[7]); let __text = ::ui_lang_runtime::selectable_text(__text); ::ui_lang_runtime::accessible(__text, ::ui_lang_runtime::StableId::new(&__a11y_key), ::ui_lang_runtime::Role::Label).logical_id_maybe(::core::cfg!(test).then_some(__a11y_key)).value(__text_value).into() };
#[cfg(test)]
drop(__ice_render_source);
__ice_rendered
}); __children.push({
#[cfg(test)]
let __ice_render_source_location = ::ui_lang_runtime::testing::Location::new("components/overlay.ice", 117, 1, "rendered view node");
#[cfg(test)]
let __ice_render_source = ::ui_lang_runtime::testing::push_render_source(__ice_render_source_location);
let __ice_rendered: __IceElement<'_, __DucktapeMessage> = (|| { let __slot_content: __IceElement<'_, __DucktapeMessage> = {
#[cfg(test)]
let __ice_render_source_location = ::ui_lang_runtime::testing::Location::new("screens/overlays.ice", 42, 1, "rendered view node");
#[cfg(test)]
let __ice_render_source = ::ui_lang_runtime::testing::push_render_source(__ice_render_source_location);
let __ice_rendered: __IceElement<'_, __DucktapeMessage> = { let __a11y_key = format!("{}/@button:42", __ice_node_scope); let __a11y_id = ::ui_lang_runtime::StableId::new(&__a11y_key); let __disabled = (*self.__ice_derived_mutation_busy()); let __activate = (__ice_cb_6)(); let __button_content: __IceElement<'_, __DucktapeMessage> = { let __button_inner: __IceElement<'_, __DucktapeMessage> = {
#[cfg(test)]
let __ice_render_source_location = ::ui_lang_runtime::testing::Location::new("screens/overlays.ice", 50, 1, "rendered view node");
#[cfg(test)]
let __ice_render_source = ::ui_lang_runtime::testing::push_render_source(__ice_render_source_location);
let __ice_rendered: __IceElement<'_, __DucktapeMessage> = { let __a11y_key = format!("{}/@container:50", __ice_node_scope); let __container_content: __IceElement<'_, __DucktapeMessage> = {
#[cfg(test)]
let __ice_render_source_location = ::ui_lang_runtime::testing::Location::new("screens/overlays.ice", 56, 1, "rendered view node");
#[cfg(test)]
let __ice_render_source = ::ui_lang_runtime::testing::push_render_source(__ice_render_source_location);
let __ice_rendered: __IceElement<'_, __DucktapeMessage> = { let __a11y_key = format!("{}/@text:56", __ice_node_scope); let __text_value = ("×").to_string(); let __text = ::iced::widget::text(__text_value.clone()).size(((14.0) as f32).max(f32::EPSILON).min(f32::MAX)).wrapping(::iced::widget::text::Wrapping::None).color(__ice_palette.colors[5]); let __text = ::ui_lang_runtime::selectable_text(__text); ::ui_lang_runtime::accessible(__text, ::ui_lang_runtime::StableId::new(&__a11y_key), ::ui_lang_runtime::Role::Label).logical_id_maybe(::core::cfg!(test).then_some(__a11y_key)).value(__text_value).into() };
#[cfg(test)]
drop(__ice_render_source);
__ice_rendered
}; let __container = {  ::iced::widget::container(__container_content).width(::iced::Fill).height(::iced::Fill).align_x(::iced::alignment::Horizontal::Center).align_y(::iced::alignment::Vertical::Center) }; ::ui_lang_runtime::accessible(__container, ::ui_lang_runtime::StableId::new(&__a11y_key), ::ui_lang_runtime::Role::GenericContainer).logical_id_maybe(::core::cfg!(test).then_some(__a11y_key)).into() };
#[cfg(test)]
drop(__ice_render_source);
__ice_rendered
}; ::iced::widget::container(__button_inner).width(::iced::Fill).align_x(::iced::alignment::Horizontal::Center).height(::iced::Fill).align_y(::iced::alignment::Vertical::Center).into() }; let __button = ::iced::widget::button(__button_content).width(26.0 as f32).height(26.0 as f32).padding(0.0 as f32).on_press_maybe(if __disabled { None } else { Some(__activate.clone()) }).style(move |__theme, __status| { let mut __style = ::iced::widget::button::primary(__theme, __status); let __background: Option<::iced::Color> = match __status { ::iced::widget::button::Status::Hovered => Some(__ice_palette.colors[14]), ::iced::widget::button::Status::Pressed => Some(__ice_palette.colors[39]), ::iced::widget::button::Status::Disabled => Some(::iced::Color::from_rgba8(0, 0, 0, 0.000000)), _ => Some(::iced::Color::from_rgba8(0, 0, 0, 0.000000)) }; if let Some(__background) = __background { __style.background = Some(::iced::Background::Color(__background)); } __style.text_color = __ice_palette.colors[4]; __style.border.radius = 7.0.into(); __style.background = ::std::option::Option::Some(::iced::Background::Color(::iced::Color::from_rgba8(0, 0, 0, 0.000000))); __style.text_color = __ice_palette.colors[5]; __style.border.color = ::iced::Color::from_rgba8(0, 0, 0, 0.000000); __style.border.width = 1.0 as f32; __style.border.radius = ::iced::border::Radius { top_left: ((7.0) as f32).max(0.0).min(f32::MAX), top_right: ((7.0) as f32).max(0.0).min(f32::MAX), bottom_right: ((7.0) as f32).max(0.0).min(f32::MAX), bottom_left: ((7.0) as f32).max(0.0).min(f32::MAX) }; match __status { ::iced::widget::button::Status::Hovered => { __style.background = ::std::option::Option::Some(::iced::Background::Color(__ice_palette.colors[55])); __style.text_color = __ice_palette.colors[4]; } ::iced::widget::button::Status::Pressed => { __style.background = ::std::option::Option::Some(::iced::Background::Color(__ice_palette.colors[56])); __style.text_color = __ice_palette.colors[4]; } _ => {} } if matches!(__status, ::iced::widget::button::Status::Disabled) { if let Some(::iced::Background::Color(mut __color)) = __style.background { __color.a *= 0.5; __style.background = Some(::iced::Background::Color(__color)); } __style.text_color.a *= 0.5; } __style }); ::ui_lang_runtime::accessible(__button, __a11y_id, ::ui_lang_runtime::Role::Button).logical_id_maybe(::core::cfg!(test).then_some(&*__a11y_key)).focus_id(::iced::widget::Id::from(__a11y_key)).label("Close".to_owned()).disabled(__disabled).on_activate_maybe(if __disabled { None } else { Some(__activate) }).focus_ring(__ice_palette.colors[42], 7.0).into() };
#[cfg(test)]
drop(__ice_render_source);
__ice_rendered
}; __slot_content })();
#[cfg(test)]
drop(__ice_render_source);
__ice_rendered
}); let __child_count = __children.len(); let __children = __children.into_iter().map(|__child| ::ui_lang_runtime::bounded_fill_element(__child, __child_count, true)).collect::<::std::vec::Vec<_>>(); let __layout = ::iced::widget::row(__children).spacing(::ui_lang_runtime::bounded_spacing(10.0, __child_count)).width(::iced::Fill).align_y(::iced::alignment::Vertical::Center); let __content = ::iced::widget::container(__layout); let __layout_content: __IceElement<'_, __DucktapeMessage> = __content.into(); let __a11y_key = format!("{}/@layout:105", __ice_use_scope); ::ui_lang_runtime::accessible(__layout_content, ::ui_lang_runtime::StableId::new(&__a11y_key), ::ui_lang_runtime::Role::GenericContainer).logical_id_maybe(::core::cfg!(test).then_some(__a11y_key)).into() };
#[cfg(test)]
drop(__ice_render_source);
__ice_rendered
}); __children.push({
#[cfg(test)]
let __ice_render_source_location = ::ui_lang_runtime::testing::Location::new("components/overlay.ice", 118, 1, "rendered view node");
#[cfg(test)]
let __ice_render_source = ::ui_lang_runtime::testing::push_render_source(__ice_render_source_location);
let __ice_rendered: __IceElement<'_, __DucktapeMessage> = (|| { let __slot_content: __IceElement<'_, __DucktapeMessage> = {
#[cfg(test)]
let __ice_render_source_location = ::ui_lang_runtime::testing::Location::new("screens/overlays.ice", 65, 1, "rendered view node");
#[cfg(test)]
let __ice_render_source = ::ui_lang_runtime::testing::push_render_source(__ice_render_source_location);
let __ice_rendered: __IceElement<'_, __DucktapeMessage> = { let mut __children: ::std::vec::Vec<__IceElement<'_, __DucktapeMessage>> = ::std::vec::Vec::new(); __children.push({
#[cfg(test)]
let __ice_render_source_location = ::ui_lang_runtime::testing::Location::new("screens/overlays.ice", 66, 1, "rendered view node");
#[cfg(test)]
let __ice_render_source = ::ui_lang_runtime::testing::push_render_source(__ice_render_source_location);
let __ice_rendered: __IceElement<'_, __DucktapeMessage> = { let __a11y_key = format!("{}/@text:66", __ice_node_scope); let __text_value = ("The channel is created immediately — there is no proposal and no approval step.").to_string(); let __text = ::iced::widget::text(__text_value.clone()).size(((12.0) as f32).max(f32::EPSILON).min(f32::MAX)).width(::iced::Fill).line_height(::iced::widget::text::LineHeight::Relative(((1.5) as f32).max(f32::EPSILON).min(f32::MAX))).color(__ice_palette.colors[70]); let __text = ::ui_lang_runtime::selectable_text(__text); ::ui_lang_runtime::accessible(__text, ::ui_lang_runtime::StableId::new(&__a11y_key), ::ui_lang_runtime::Role::Label).logical_id_maybe(::core::cfg!(test).then_some(__a11y_key)).value(__text_value).into() };
#[cfg(test)]
drop(__ice_render_source);
__ice_rendered
}); __children.push({
#[cfg(test)]
let __ice_render_source_location = ::ui_lang_runtime::testing::Location::new("screens/overlays.ice", 72, 1, "rendered view node");
#[cfg(test)]
let __ice_render_source = ::ui_lang_runtime::testing::push_render_source(__ice_render_source_location);
let __ice_rendered: __IceElement<'_, __DucktapeMessage> = { let mut __children: ::std::vec::Vec<__IceElement<'_, __DucktapeMessage>> = ::std::vec::Vec::new(); __children.push({
#[cfg(test)]
let __ice_render_source_location = ::ui_lang_runtime::testing::Location::new("screens/overlays.ice", 73, 1, "rendered view node");
#[cfg(test)]
let __ice_render_source = ::ui_lang_runtime::testing::push_render_source(__ice_render_source_location);
let __ice_rendered: __IceElement<'_, __DucktapeMessage> = { let __ice_component_eyebrow_scope_6633 = format!("{}/Eyebrow@6633", __ice_node_scope); ::ui_lang_runtime::rev_memo(1028u64, (__ice_palette.name), ::ui_lang_runtime::grow_stack(|| self.__ice_component_use_85(__ice_palette, __ice_component_eyebrow_scope_6633))).into() };
#[cfg(test)]
drop(__ice_render_source);
__ice_rendered
}); __children.push({
#[cfg(test)]
let __ice_render_source_location = ::ui_lang_runtime::testing::Location::new("screens/overlays.ice", 74, 1, "rendered view node");
#[cfg(test)]
let __ice_render_source = ::ui_lang_runtime::testing::push_render_source(__ice_render_source_location);
let __ice_rendered: __IceElement<'_, __DucktapeMessage> = { let __a11y_key = format!("{}/@container:74", __ice_node_scope); let __container_content: __IceElement<'_, __DucktapeMessage> = {
#[cfg(test)]
let __ice_render_source_location = ::ui_lang_runtime::testing::Location::new("screens/overlays.ice", 85, 1, "rendered view node");
#[cfg(test)]
let __ice_render_source = ::ui_lang_runtime::testing::push_render_source(__ice_render_source_location);
let __ice_rendered: __IceElement<'_, __DucktapeMessage> = { let mut __children: ::std::vec::Vec<__IceElement<'_, __DucktapeMessage>> = ::std::vec::Vec::new(); __children.push({
#[cfg(test)]
let __ice_render_source_location = ::ui_lang_runtime::testing::Location::new("screens/overlays.ice", 90, 1, "rendered view node");
#[cfg(test)]
let __ice_render_source = ::ui_lang_runtime::testing::push_render_source(__ice_render_source_location);
let __ice_rendered: __IceElement<'_, __DucktapeMessage> = { let __a11y_key = format!("{}/@text:90", __ice_node_scope); let __text_value = ("#").to_string(); let __text = ::iced::widget::text(__text_value.clone()).size(((14.0) as f32).max(f32::EPSILON).min(f32::MAX)).wrapping(::iced::widget::text::Wrapping::None).font(::iced::Font { family: ::iced::font::Family::Name("Geist Mono"), weight: ::iced::font::Weight::Medium, stretch: ::iced::font::Stretch::Normal, style: ::iced::font::Style::Normal }).color(__ice_palette.colors[73]); let __text = ::ui_lang_runtime::selectable_text(__text); ::ui_lang_runtime::accessible(__text, ::ui_lang_runtime::StableId::new(&__a11y_key), ::ui_lang_runtime::Role::Label).logical_id_maybe(::core::cfg!(test).then_some(__a11y_key)).value(__text_value).into() };
#[cfg(test)]
drop(__ice_render_source);
__ice_rendered
}); __children.push({
#[cfg(test)]
let __ice_render_source_location = ::ui_lang_runtime::testing::Location::new("screens/overlays.ice", 96, 1, "rendered view node");
#[cfg(test)]
let __ice_render_source = ::ui_lang_runtime::testing::push_render_source(__ice_render_source_location);
let __ice_rendered: __IceElement<'_, __DucktapeMessage> = { let __ice_node_scope = format!("{}/new-channel", __ice_node_scope); { let __a11y_key = __ice_node_scope.clone(); let __a11y_id = ::ui_lang_runtime::StableId::new(&__a11y_key); let __disabled = ((self.loading || (*self.__ice_derived_mutation_busy())) || (!self.connected)); let __secure = false; let __role = if __secure { ::ui_lang_runtime::Role::PasswordInput } else { ::ui_lang_runtime::Role::TextInput }; let __input = ::ui_lang_runtime::accessible(::iced::widget::text_input("design-review", &self.channel_draft).id(::iced::widget::Id::from(__a11y_key.clone())).padding(::iced::Padding { top: 11.0, right: 13.0, bottom: 11.0, left: 13.0 }).width(::iced::Fill).secure(__secure).width(::iced::Fill).padding(6.2 as f32).size(((13.0) as f32).max(f32::EPSILON).min(f32::MAX)).line_height(::iced::widget::text::LineHeight::Relative(((1.2) as f32).max(f32::EPSILON).min(f32::MAX))).font(::iced::Font { family: ::iced::font::Family::Name("Geist Mono"), weight: ::iced::font::Weight::Normal, stretch: ::iced::font::Stretch::Normal, style: ::iced::font::Style::Normal }).on_input_maybe(if __disabled { None } else { Some(__DucktapeMessage::__BindChannelDraft as fn(::std::string::String) -> __DucktapeMessage) }).on_submit_maybe(if __disabled { None } else { Some((__ice_cb_1)()) }).style(move |__theme, __status| { let mut __style = ::iced::widget::text_input::default(__theme, __status); __style.background = __ice_palette.colors[3].into(); __style.border.color = __ice_palette.colors[39]; __style.border.width = 1.0; __style.border.radius = 10.0.into(); __style.background = ::iced::Background::Color(::iced::Color::from_rgba8(0, 0, 0, 0.000000)); __style.border.color = ::iced::Color::from_rgba8(0, 0, 0, 0.000000); __style.border.width = 0.0 as f32; __style.border.radius = ::iced::border::Radius { top_left: ((0.0) as f32).max(0.0).min(f32::MAX), top_right: ((0.0) as f32).max(0.0).min(f32::MAX), bottom_right: ((0.0) as f32).max(0.0).min(f32::MAX), bottom_left: ((0.0) as f32).max(0.0).min(f32::MAX) }; __style.placeholder = __ice_palette.colors[72]; __style.value = __ice_palette.colors[4]; __style.selection = { let mut __color = __ice_palette.colors[4]; __color.a = 0.180000; __color }; if matches!(__status, ::iced::widget::text_input::Status::Focused { .. }) { __style.border.color = __ice_palette.colors[42]; } match __status { ::iced::widget::text_input::Status::Hovered => { __style.background = ::iced::Background::Color(::iced::Color::from_rgba8(0, 0, 0, 0.000000)); __style.border.color = ::iced::Color::from_rgba8(0, 0, 0, 0.000000); } ::iced::widget::text_input::Status::Disabled => { __style.value = __ice_palette.colors[5]; } _ => {} } __style }), __a11y_id, __role).logical_id_maybe(::core::cfg!(test).then_some(&*__a11y_key)).focus_id(::iced::widget::Id::from(__a11y_key)).label("New channel name".to_owned()).value_maybe((!__secure).then(|| (self.channel_draft).to_owned())).disabled(__disabled); __input.into() } };
#[cfg(test)]
drop(__ice_render_source);
__ice_rendered
}); let __child_count = __children.len(); let __children = __children.into_iter().map(|__child| ::ui_lang_runtime::bounded_fill_element(__child, __child_count, true)).collect::<::std::vec::Vec<_>>(); let __layout = ::iced::widget::row(__children).spacing(::ui_lang_runtime::bounded_spacing(7.0, __child_count)).width(::iced::Fill).align_y(::iced::alignment::Vertical::Center); let __content = ::iced::widget::container(__layout); let __layout_content: __IceElement<'_, __DucktapeMessage> = __content.into(); let __a11y_key = format!("{}/@layout:85", __ice_node_scope); ::ui_lang_runtime::accessible(__layout_content, ::ui_lang_runtime::StableId::new(&__a11y_key), ::ui_lang_runtime::Role::GenericContainer).logical_id_maybe(::core::cfg!(test).then_some(__a11y_key)).into() };
#[cfg(test)]
drop(__ice_render_source);
__ice_rendered
}; let __container = {  ::iced::widget::container(__container_content).padding(::ui_lang_runtime::bounded_padding(2.0, 11.0, 2.0, 11.0)).width(::iced::Fill).style(move |__theme| { let mut __style = ::iced::widget::container::Style::default(); __style.background = ::std::option::Option::Some(::iced::Background::Color(__ice_palette.colors[3])); __style.border.color = __ice_palette.colors[7]; __style.border.width = 1.5 as f32; __style.border.radius = ::iced::border::Radius { top_left: ((9.0) as f32).max(0.0).min(f32::MAX), top_right: ((9.0) as f32).max(0.0).min(f32::MAX), bottom_right: ((9.0) as f32).max(0.0).min(f32::MAX), bottom_left: ((9.0) as f32).max(0.0).min(f32::MAX) }; __style }) }; ::ui_lang_runtime::accessible(__container, ::ui_lang_runtime::StableId::new(&__a11y_key), ::ui_lang_runtime::Role::GenericContainer).logical_id_maybe(::core::cfg!(test).then_some(__a11y_key)).into() };
#[cfg(test)]
drop(__ice_render_source);
__ice_rendered
}); let __child_count = __children.len(); let __children = __children.into_iter().map(|__child| ::ui_lang_runtime::bounded_fill_element(__child, __child_count, false)).collect::<::std::vec::Vec<_>>(); let __layout = ::iced::widget::column(__children).spacing(::ui_lang_runtime::bounded_spacing(6.0, __child_count)).width(::iced::Fill); let __content = ::iced::widget::container(__layout); let __layout_content: __IceElement<'_, __DucktapeMessage> = __content.into(); let __a11y_key = format!("{}/@layout:72", __ice_node_scope); ::ui_lang_runtime::accessible(__layout_content, ::ui_lang_runtime::StableId::new(&__a11y_key), ::ui_lang_runtime::Role::GenericContainer).logical_id_maybe(::core::cfg!(test).then_some(__a11y_key)).into() };
#[cfg(test)]
drop(__ice_render_source);
__ice_rendered
}); __children.push({
#[cfg(test)]
let __ice_render_source_location = ::ui_lang_runtime::testing::Location::new("screens/overlays.ice", 111, 1, "rendered view node");
#[cfg(test)]
let __ice_render_source = ::ui_lang_runtime::testing::push_render_source(__ice_render_source_location);
let __ice_rendered: __IceElement<'_, __DucktapeMessage> = { let mut __children: ::std::vec::Vec<__IceElement<'_, __DucktapeMessage>> = ::std::vec::Vec::new(); __children.push({
#[cfg(test)]
let __ice_render_source_location = ::ui_lang_runtime::testing::Location::new("screens/overlays.ice", 112, 1, "rendered view node");
#[cfg(test)]
let __ice_render_source = ::ui_lang_runtime::testing::push_render_source(__ice_render_source_location);
let __ice_rendered: __IceElement<'_, __DucktapeMessage> = { let __ice_component_eyebrow_scope_6672 = format!("{}/Eyebrow@6672", __ice_node_scope); ::ui_lang_runtime::rev_memo(1034u64, (__ice_palette.name), ::ui_lang_runtime::grow_stack(|| self.__ice_component_use_86(__ice_palette, __ice_component_eyebrow_scope_6672))).into() };
#[cfg(test)]
drop(__ice_render_source);
__ice_rendered
}); __children.push({
#[cfg(test)]
let __ice_render_source_location = ::ui_lang_runtime::testing::Location::new("screens/overlays.ice", 116, 1, "rendered view node");
#[cfg(test)]
let __ice_render_source = ::ui_lang_runtime::testing::push_render_source(__ice_render_source_location);
let __ice_rendered: __IceElement<'_, __DucktapeMessage> = { let mut __children: ::std::vec::Vec<__IceElement<'_, __DucktapeMessage>> = ::std::vec::Vec::new(); if (!self.channel_create_members_only) { __children.push({
#[cfg(test)]
let __ice_render_source_location = ::ui_lang_runtime::testing::Location::new("screens/overlays.ice", 122, 1, "rendered view node");
#[cfg(test)]
let __ice_render_source = ::ui_lang_runtime::testing::push_render_source(__ice_render_source_location);
let __ice_rendered: __IceElement<'_, __DucktapeMessage> = { let __a11y_key = format!("{}/@container:122", __ice_node_scope); let __container_content: __IceElement<'_, __DucktapeMessage> = {
#[cfg(test)]
let __ice_render_source_location = ::ui_lang_runtime::testing::Location::new("screens/overlays.ice", 133, 1, "rendered view node");
#[cfg(test)]
let __ice_render_source = ::ui_lang_runtime::testing::push_render_source(__ice_render_source_location);
let __ice_rendered: __IceElement<'_, __DucktapeMessage> = { let mut __children: ::std::vec::Vec<__IceElement<'_, __DucktapeMessage>> = ::std::vec::Vec::new(); __children.push({
#[cfg(test)]
let __ice_render_source_location = ::ui_lang_runtime::testing::Location::new("screens/overlays.ice", 134, 1, "rendered view node");
#[cfg(test)]
let __ice_render_source = ::ui_lang_runtime::testing::push_render_source(__ice_render_source_location);
let __ice_rendered: __IceElement<'_, __DucktapeMessage> = { let __a11y_key = format!("{}/@text:134", __ice_node_scope); let __text_value = ("Open").to_string(); let __text = ::iced::widget::text(__text_value.clone()).size(((12.5) as f32).max(f32::EPSILON).min(f32::MAX)).wrapping(::iced::widget::text::Wrapping::None).font(::iced::Font { family: ::iced::font::Family::Name("Geist"), weight: ::iced::font::Weight::Medium, stretch: ::iced::font::Stretch::Normal, style: ::iced::font::Style::Normal }).color(__ice_palette.colors[15]); let __text = ::ui_lang_runtime::selectable_text(__text); ::ui_lang_runtime::accessible(__text, ::ui_lang_runtime::StableId::new(&__a11y_key), ::ui_lang_runtime::Role::Label).logical_id_maybe(::core::cfg!(test).then_some(__a11y_key)).value(__text_value).into() };
#[cfg(test)]
drop(__ice_render_source);
__ice_rendered
}); __children.push({
#[cfg(test)]
let __ice_render_source_location = ::ui_lang_runtime::testing::Location::new("screens/overlays.ice", 140, 1, "rendered view node");
#[cfg(test)]
let __ice_render_source = ::ui_lang_runtime::testing::push_render_source(__ice_render_source_location);
let __ice_rendered: __IceElement<'_, __DucktapeMessage> = { let __a11y_key = format!("{}/@text:140", __ice_node_scope); let __text_value = ("any member posts").to_string(); let __text = ::iced::widget::text(__text_value.clone()).size(((11.0) as f32).max(f32::EPSILON).min(f32::MAX)).wrapping(::iced::widget::text::Wrapping::None).color(__ice_palette.colors[70]); let __text = ::ui_lang_runtime::selectable_text(__text); ::ui_lang_runtime::accessible(__text, ::ui_lang_runtime::StableId::new(&__a11y_key), ::ui_lang_runtime::Role::Label).logical_id_maybe(::core::cfg!(test).then_some(__a11y_key)).value(__text_value).into() };
#[cfg(test)]
drop(__ice_render_source);
__ice_rendered
}); let __child_count = __children.len(); let __children = __children.into_iter().map(|__child| ::ui_lang_runtime::bounded_fill_element(__child, __child_count, false)).collect::<::std::vec::Vec<_>>(); let __layout = ::iced::widget::column(__children).spacing(::ui_lang_runtime::bounded_spacing(2.0, __child_count)).width(::iced::Fill); let __content = ::iced::widget::container(__layout); let __layout_content: __IceElement<'_, __DucktapeMessage> = __content.into(); let __a11y_key = format!("{}/@layout:133", __ice_node_scope); ::ui_lang_runtime::accessible(__layout_content, ::ui_lang_runtime::StableId::new(&__a11y_key), ::ui_lang_runtime::Role::GenericContainer).logical_id_maybe(::core::cfg!(test).then_some(__a11y_key)).into() };
#[cfg(test)]
drop(__ice_render_source);
__ice_rendered
}; let __container = {  ::iced::widget::container(__container_content).padding(::ui_lang_runtime::bounded_padding(8.0, 12.0, 8.0, 12.0)).width(::iced::Fill).style(move |__theme| { let mut __style = ::iced::widget::container::Style::default(); __style.background = ::std::option::Option::Some(::iced::Background::Color(__ice_palette.colors[6])); __style.border.color = __ice_palette.colors[7]; __style.border.width = 1.5 as f32; __style.border.radius = ::iced::border::Radius { top_left: ((9.0) as f32).max(0.0).min(f32::MAX), top_right: ((9.0) as f32).max(0.0).min(f32::MAX), bottom_right: ((9.0) as f32).max(0.0).min(f32::MAX), bottom_left: ((9.0) as f32).max(0.0).min(f32::MAX) }; __style }) }; ::ui_lang_runtime::accessible(__container, ::ui_lang_runtime::StableId::new(&__a11y_key), ::ui_lang_runtime::Role::GenericContainer).logical_id_maybe(::core::cfg!(test).then_some(__a11y_key)).into() };
#[cfg(test)]
drop(__ice_render_source);
__ice_rendered
}); } if self.channel_create_members_only { __children.push({
#[cfg(test)]
let __ice_render_source_location = ::ui_lang_runtime::testing::Location::new("screens/overlays.ice", 146, 1, "rendered view node");
#[cfg(test)]
let __ice_render_source = ::ui_lang_runtime::testing::push_render_source(__ice_render_source_location);
let __ice_rendered: __IceElement<'_, __DucktapeMessage> = { let __a11y_key = format!("{}/@button:146", __ice_node_scope); let __a11y_id = ::ui_lang_runtime::StableId::new(&__a11y_key); let __disabled = false; let __activate = (__ice_cb_7)(); let __button_content: __IceElement<'_, __DucktapeMessage> = {
#[cfg(test)]
let __ice_render_source_location = ::ui_lang_runtime::testing::Location::new("screens/overlays.ice", 152, 1, "rendered view node");
#[cfg(test)]
let __ice_render_source = ::ui_lang_runtime::testing::push_render_source(__ice_render_source_location);
let __ice_rendered: __IceElement<'_, __DucktapeMessage> = { let __a11y_key = format!("{}/@container:152", __ice_node_scope); let __container_content: __IceElement<'_, __DucktapeMessage> = {
#[cfg(test)]
let __ice_render_source_location = ::ui_lang_runtime::testing::Location::new("screens/overlays.ice", 159, 1, "rendered view node");
#[cfg(test)]
let __ice_render_source = ::ui_lang_runtime::testing::push_render_source(__ice_render_source_location);
let __ice_rendered: __IceElement<'_, __DucktapeMessage> = { let mut __children: ::std::vec::Vec<__IceElement<'_, __DucktapeMessage>> = ::std::vec::Vec::new(); __children.push({
#[cfg(test)]
let __ice_render_source_location = ::ui_lang_runtime::testing::Location::new("screens/overlays.ice", 160, 1, "rendered view node");
#[cfg(test)]
let __ice_render_source = ::ui_lang_runtime::testing::push_render_source(__ice_render_source_location);
let __ice_rendered: __IceElement<'_, __DucktapeMessage> = { let __a11y_key = format!("{}/@text:160", __ice_node_scope); let __text_value = ("Open").to_string(); let __text = ::iced::widget::text(__text_value.clone()).size(((12.5) as f32).max(f32::EPSILON).min(f32::MAX)).wrapping(::iced::widget::text::Wrapping::None).font(::iced::Font { family: ::iced::font::Family::Name("Geist"), weight: ::iced::font::Weight::Medium, stretch: ::iced::font::Stretch::Normal, style: ::iced::font::Style::Normal }).color(__ice_palette.colors[15]); let __text = ::ui_lang_runtime::selectable_text(__text); ::ui_lang_runtime::accessible(__text, ::ui_lang_runtime::StableId::new(&__a11y_key), ::ui_lang_runtime::Role::Label).logical_id_maybe(::core::cfg!(test).then_some(__a11y_key)).value(__text_value).into() };
#[cfg(test)]
drop(__ice_render_source);
__ice_rendered
}); __children.push({
#[cfg(test)]
let __ice_render_source_location = ::ui_lang_runtime::testing::Location::new("screens/overlays.ice", 166, 1, "rendered view node");
#[cfg(test)]
let __ice_render_source = ::ui_lang_runtime::testing::push_render_source(__ice_render_source_location);
let __ice_rendered: __IceElement<'_, __DucktapeMessage> = { let __a11y_key = format!("{}/@text:166", __ice_node_scope); let __text_value = ("any member posts").to_string(); let __text = ::iced::widget::text(__text_value.clone()).size(((11.0) as f32).max(f32::EPSILON).min(f32::MAX)).wrapping(::iced::widget::text::Wrapping::None).color(__ice_palette.colors[70]); let __text = ::ui_lang_runtime::selectable_text(__text); ::ui_lang_runtime::accessible(__text, ::ui_lang_runtime::StableId::new(&__a11y_key), ::ui_lang_runtime::Role::Label).logical_id_maybe(::core::cfg!(test).then_some(__a11y_key)).value(__text_value).into() };
#[cfg(test)]
drop(__ice_render_source);
__ice_rendered
}); let __child_count = __children.len(); let __children = __children.into_iter().map(|__child| ::ui_lang_runtime::bounded_fill_element(__child, __child_count, false)).collect::<::std::vec::Vec<_>>(); let __layout = ::iced::widget::column(__children).spacing(::ui_lang_runtime::bounded_spacing(2.0, __child_count)).width(::iced::Fill); let __content = ::iced::widget::container(__layout); let __layout_content: __IceElement<'_, __DucktapeMessage> = __content.into(); let __a11y_key = format!("{}/@layout:159", __ice_node_scope); ::ui_lang_runtime::accessible(__layout_content, ::ui_lang_runtime::StableId::new(&__a11y_key), ::ui_lang_runtime::Role::GenericContainer).logical_id_maybe(::core::cfg!(test).then_some(__a11y_key)).into() };
#[cfg(test)]
drop(__ice_render_source);
__ice_rendered
}; let __container = {  ::iced::widget::container(__container_content).padding(::ui_lang_runtime::bounded_padding(8.0, 12.0, 8.0, 12.0)).width(::iced::Fill) }; ::ui_lang_runtime::accessible(__container, ::ui_lang_runtime::StableId::new(&__a11y_key), ::ui_lang_runtime::Role::GenericContainer).logical_id_maybe(::core::cfg!(test).then_some(__a11y_key)).into() };
#[cfg(test)]
drop(__ice_render_source);
__ice_rendered
}; let __button = ::iced::widget::button(__button_content).padding(::iced::Padding { top: 7.0, right: 12.0, bottom: 7.0, left: 12.0 }).width(::iced::Fill).padding(0.0 as f32).on_press_maybe(if __disabled { None } else { Some(__activate.clone()) }).style(move |__theme, __status| { let mut __style = ::iced::widget::button::primary(__theme, __status); let __background: Option<::iced::Color> = match __status { ::iced::widget::button::Status::Hovered => Some(__ice_palette.colors[14]), ::iced::widget::button::Status::Pressed => Some(__ice_palette.colors[39]), ::iced::widget::button::Status::Disabled => Some(::iced::Color::from_rgba8(0, 0, 0, 0.000000)), _ => Some(::iced::Color::from_rgba8(0, 0, 0, 0.000000)) }; if let Some(__background) = __background { __style.background = Some(::iced::Background::Color(__background)); } __style.text_color = __ice_palette.colors[4]; __style.border.radius = 8.0.into(); __style.background = ::std::option::Option::Some(::iced::Background::Color(__ice_palette.colors[3])); __style.text_color = __ice_palette.colors[4]; __style.border.color = __ice_palette.colors[39]; __style.border.width = 1.5 as f32; __style.border.radius = ::iced::border::Radius { top_left: ((9.0) as f32).max(0.0).min(f32::MAX), top_right: ((9.0) as f32).max(0.0).min(f32::MAX), bottom_right: ((9.0) as f32).max(0.0).min(f32::MAX), bottom_left: ((9.0) as f32).max(0.0).min(f32::MAX) }; match __status { ::iced::widget::button::Status::Hovered => { __style.background = ::std::option::Option::Some(::iced::Background::Color(__ice_palette.colors[6])); __style.text_color = __ice_palette.colors[4]; __style.border.color = __ice_palette.colors[40]; } ::iced::widget::button::Status::Pressed => { __style.background = ::std::option::Option::Some(::iced::Background::Color(__ice_palette.colors[55])); __style.text_color = __ice_palette.colors[4]; } _ => {} } if matches!(__status, ::iced::widget::button::Status::Disabled) { if let Some(::iced::Background::Color(mut __color)) = __style.background { __color.a *= 0.5; __style.background = Some(::iced::Background::Color(__color)); } __style.text_color.a *= 0.5; } __style }); ::ui_lang_runtime::accessible(__button, __a11y_id, ::ui_lang_runtime::Role::Button).logical_id_maybe(::core::cfg!(test).then_some(&*__a11y_key)).focus_id(::iced::widget::Id::from(__a11y_key)).label("Open posting".to_owned()).disabled(__disabled).on_activate_maybe(if __disabled { None } else { Some(__activate) }).focus_ring(__ice_palette.colors[42], 8.0).into() };
#[cfg(test)]
drop(__ice_render_source);
__ice_rendered
}); } if self.channel_create_members_only { __children.push({
#[cfg(test)]
let __ice_render_source_location = ::ui_lang_runtime::testing::Location::new("screens/overlays.ice", 175, 1, "rendered view node");
#[cfg(test)]
let __ice_render_source = ::ui_lang_runtime::testing::push_render_source(__ice_render_source_location);
let __ice_rendered: __IceElement<'_, __DucktapeMessage> = { let __a11y_key = format!("{}/@container:175", __ice_node_scope); let __container_content: __IceElement<'_, __DucktapeMessage> = {
#[cfg(test)]
let __ice_render_source_location = ::ui_lang_runtime::testing::Location::new("screens/overlays.ice", 186, 1, "rendered view node");
#[cfg(test)]
let __ice_render_source = ::ui_lang_runtime::testing::push_render_source(__ice_render_source_location);
let __ice_rendered: __IceElement<'_, __DucktapeMessage> = { let mut __children: ::std::vec::Vec<__IceElement<'_, __DucktapeMessage>> = ::std::vec::Vec::new(); __children.push({
#[cfg(test)]
let __ice_render_source_location = ::ui_lang_runtime::testing::Location::new("screens/overlays.ice", 187, 1, "rendered view node");
#[cfg(test)]
let __ice_render_source = ::ui_lang_runtime::testing::push_render_source(__ice_render_source_location);
let __ice_rendered: __IceElement<'_, __DucktapeMessage> = { let __a11y_key = format!("{}/@text:187", __ice_node_scope); let __text_value = ("Members only").to_string(); let __text = ::iced::widget::text(__text_value.clone()).size(((12.5) as f32).max(f32::EPSILON).min(f32::MAX)).wrapping(::iced::widget::text::Wrapping::None).font(::iced::Font { family: ::iced::font::Family::Name("Geist"), weight: ::iced::font::Weight::Medium, stretch: ::iced::font::Stretch::Normal, style: ::iced::font::Style::Normal }).color(__ice_palette.colors[15]); let __text = ::ui_lang_runtime::selectable_text(__text); ::ui_lang_runtime::accessible(__text, ::ui_lang_runtime::StableId::new(&__a11y_key), ::ui_lang_runtime::Role::Label).logical_id_maybe(::core::cfg!(test).then_some(__a11y_key)).value(__text_value).into() };
#[cfg(test)]
drop(__ice_render_source);
__ice_rendered
}); __children.push({
#[cfg(test)]
let __ice_render_source_location = ::ui_lang_runtime::testing::Location::new("screens/overlays.ice", 193, 1, "rendered view node");
#[cfg(test)]
let __ice_render_source = ::ui_lang_runtime::testing::push_render_source(__ice_render_source_location);
let __ice_rendered: __IceElement<'_, __DucktapeMessage> = { let __a11y_key = format!("{}/@text:193", __ice_node_scope); let __text_value = ("added members post").to_string(); let __text = ::iced::widget::text(__text_value.clone()).size(((11.0) as f32).max(f32::EPSILON).min(f32::MAX)).wrapping(::iced::widget::text::Wrapping::None).color(__ice_palette.colors[70]); let __text = ::ui_lang_runtime::selectable_text(__text); ::ui_lang_runtime::accessible(__text, ::ui_lang_runtime::StableId::new(&__a11y_key), ::ui_lang_runtime::Role::Label).logical_id_maybe(::core::cfg!(test).then_some(__a11y_key)).value(__text_value).into() };
#[cfg(test)]
drop(__ice_render_source);
__ice_rendered
}); let __child_count = __children.len(); let __children = __children.into_iter().map(|__child| ::ui_lang_runtime::bounded_fill_element(__child, __child_count, false)).collect::<::std::vec::Vec<_>>(); let __layout = ::iced::widget::column(__children).spacing(::ui_lang_runtime::bounded_spacing(2.0, __child_count)).width(::iced::Fill); let __content = ::iced::widget::container(__layout); let __layout_content: __IceElement<'_, __DucktapeMessage> = __content.into(); let __a11y_key = format!("{}/@layout:186", __ice_node_scope); ::ui_lang_runtime::accessible(__layout_content, ::ui_lang_runtime::StableId::new(&__a11y_key), ::ui_lang_runtime::Role::GenericContainer).logical_id_maybe(::core::cfg!(test).then_some(__a11y_key)).into() };
#[cfg(test)]
drop(__ice_render_source);
__ice_rendered
}; let __container = {  ::iced::widget::container(__container_content).padding(::ui_lang_runtime::bounded_padding(8.0, 12.0, 8.0, 12.0)).width(::iced::Fill).style(move |__theme| { let mut __style = ::iced::widget::container::Style::default(); __style.background = ::std::option::Option::Some(::iced::Background::Color(__ice_palette.colors[6])); __style.border.color = __ice_palette.colors[7]; __style.border.width = 1.5 as f32; __style.border.radius = ::iced::border::Radius { top_left: ((9.0) as f32).max(0.0).min(f32::MAX), top_right: ((9.0) as f32).max(0.0).min(f32::MAX), bottom_right: ((9.0) as f32).max(0.0).min(f32::MAX), bottom_left: ((9.0) as f32).max(0.0).min(f32::MAX) }; __style }) }; ::ui_lang_runtime::accessible(__container, ::ui_lang_runtime::StableId::new(&__a11y_key), ::ui_lang_runtime::Role::GenericContainer).logical_id_maybe(::core::cfg!(test).then_some(__a11y_key)).into() };
#[cfg(test)]
drop(__ice_render_source);
__ice_rendered
}); } if (!self.channel_create_members_only) { __children.push({
#[cfg(test)]
let __ice_render_source_location = ::ui_lang_runtime::testing::Location::new("screens/overlays.ice", 199, 1, "rendered view node");
#[cfg(test)]
let __ice_render_source = ::ui_lang_runtime::testing::push_render_source(__ice_render_source_location);
let __ice_rendered: __IceElement<'_, __DucktapeMessage> = { let __a11y_key = format!("{}/@button:199", __ice_node_scope); let __a11y_id = ::ui_lang_runtime::StableId::new(&__a11y_key); let __disabled = false; let __activate = (__ice_cb_7)(); let __button_content: __IceElement<'_, __DucktapeMessage> = {
#[cfg(test)]
let __ice_render_source_location = ::ui_lang_runtime::testing::Location::new("screens/overlays.ice", 205, 1, "rendered view node");
#[cfg(test)]
let __ice_render_source = ::ui_lang_runtime::testing::push_render_source(__ice_render_source_location);
let __ice_rendered: __IceElement<'_, __DucktapeMessage> = { let __a11y_key = format!("{}/@container:205", __ice_node_scope); let __container_content: __IceElement<'_, __DucktapeMessage> = {
#[cfg(test)]
let __ice_render_source_location = ::ui_lang_runtime::testing::Location::new("screens/overlays.ice", 212, 1, "rendered view node");
#[cfg(test)]
let __ice_render_source = ::ui_lang_runtime::testing::push_render_source(__ice_render_source_location);
let __ice_rendered: __IceElement<'_, __DucktapeMessage> = { let mut __children: ::std::vec::Vec<__IceElement<'_, __DucktapeMessage>> = ::std::vec::Vec::new(); __children.push({
#[cfg(test)]
let __ice_render_source_location = ::ui_lang_runtime::testing::Location::new("screens/overlays.ice", 213, 1, "rendered view node");
#[cfg(test)]
let __ice_render_source = ::ui_lang_runtime::testing::push_render_source(__ice_render_source_location);
let __ice_rendered: __IceElement<'_, __DucktapeMessage> = { let __a11y_key = format!("{}/@text:213", __ice_node_scope); let __text_value = ("Members only").to_string(); let __text = ::iced::widget::text(__text_value.clone()).size(((12.5) as f32).max(f32::EPSILON).min(f32::MAX)).wrapping(::iced::widget::text::Wrapping::None).font(::iced::Font { family: ::iced::font::Family::Name("Geist"), weight: ::iced::font::Weight::Medium, stretch: ::iced::font::Stretch::Normal, style: ::iced::font::Style::Normal }).color(__ice_palette.colors[15]); let __text = ::ui_lang_runtime::selectable_text(__text); ::ui_lang_runtime::accessible(__text, ::ui_lang_runtime::StableId::new(&__a11y_key), ::ui_lang_runtime::Role::Label).logical_id_maybe(::core::cfg!(test).then_some(__a11y_key)).value(__text_value).into() };
#[cfg(test)]
drop(__ice_render_source);
__ice_rendered
}); __children.push({
#[cfg(test)]
let __ice_render_source_location = ::ui_lang_runtime::testing::Location::new("screens/overlays.ice", 219, 1, "rendered view node");
#[cfg(test)]
let __ice_render_source = ::ui_lang_runtime::testing::push_render_source(__ice_render_source_location);
let __ice_rendered: __IceElement<'_, __DucktapeMessage> = { let __a11y_key = format!("{}/@text:219", __ice_node_scope); let __text_value = ("added members post").to_string(); let __text = ::iced::widget::text(__text_value.clone()).size(((11.0) as f32).max(f32::EPSILON).min(f32::MAX)).wrapping(::iced::widget::text::Wrapping::None).color(__ice_palette.colors[70]); let __text = ::ui_lang_runtime::selectable_text(__text); ::ui_lang_runtime::accessible(__text, ::ui_lang_runtime::StableId::new(&__a11y_key), ::ui_lang_runtime::Role::Label).logical_id_maybe(::core::cfg!(test).then_some(__a11y_key)).value(__text_value).into() };
#[cfg(test)]
drop(__ice_render_source);
__ice_rendered
}); let __child_count = __children.len(); let __children = __children.into_iter().map(|__child| ::ui_lang_runtime::bounded_fill_element(__child, __child_count, false)).collect::<::std::vec::Vec<_>>(); let __layout = ::iced::widget::column(__children).spacing(::ui_lang_runtime::bounded_spacing(2.0, __child_count)).width(::iced::Fill); let __content = ::iced::widget::container(__layout); let __layout_content: __IceElement<'_, __DucktapeMessage> = __content.into(); let __a11y_key = format!("{}/@layout:212", __ice_node_scope); ::ui_lang_runtime::accessible(__layout_content, ::ui_lang_runtime::StableId::new(&__a11y_key), ::ui_lang_runtime::Role::GenericContainer).logical_id_maybe(::core::cfg!(test).then_some(__a11y_key)).into() };
#[cfg(test)]
drop(__ice_render_source);
__ice_rendered
}; let __container = {  ::iced::widget::container(__container_content).padding(::ui_lang_runtime::bounded_padding(8.0, 12.0, 8.0, 12.0)).width(::iced::Fill) }; ::ui_lang_runtime::accessible(__container, ::ui_lang_runtime::StableId::new(&__a11y_key), ::ui_lang_runtime::Role::GenericContainer).logical_id_maybe(::core::cfg!(test).then_some(__a11y_key)).into() };
#[cfg(test)]
drop(__ice_render_source);
__ice_rendered
}; let __button = ::iced::widget::button(__button_content).padding(::iced::Padding { top: 7.0, right: 12.0, bottom: 7.0, left: 12.0 }).width(::iced::Fill).padding(0.0 as f32).on_press_maybe(if __disabled { None } else { Some(__activate.clone()) }).style(move |__theme, __status| { let mut __style = ::iced::widget::button::primary(__theme, __status); let __background: Option<::iced::Color> = match __status { ::iced::widget::button::Status::Hovered => Some(__ice_palette.colors[14]), ::iced::widget::button::Status::Pressed => Some(__ice_palette.colors[39]), ::iced::widget::button::Status::Disabled => Some(::iced::Color::from_rgba8(0, 0, 0, 0.000000)), _ => Some(::iced::Color::from_rgba8(0, 0, 0, 0.000000)) }; if let Some(__background) = __background { __style.background = Some(::iced::Background::Color(__background)); } __style.text_color = __ice_palette.colors[4]; __style.border.radius = 8.0.into(); __style.background = ::std::option::Option::Some(::iced::Background::Color(__ice_palette.colors[3])); __style.text_color = __ice_palette.colors[4]; __style.border.color = __ice_palette.colors[39]; __style.border.width = 1.5 as f32; __style.border.radius = ::iced::border::Radius { top_left: ((9.0) as f32).max(0.0).min(f32::MAX), top_right: ((9.0) as f32).max(0.0).min(f32::MAX), bottom_right: ((9.0) as f32).max(0.0).min(f32::MAX), bottom_left: ((9.0) as f32).max(0.0).min(f32::MAX) }; match __status { ::iced::widget::button::Status::Hovered => { __style.background = ::std::option::Option::Some(::iced::Background::Color(__ice_palette.colors[6])); __style.text_color = __ice_palette.colors[4]; __style.border.color = __ice_palette.colors[40]; } ::iced::widget::button::Status::Pressed => { __style.background = ::std::option::Option::Some(::iced::Background::Color(__ice_palette.colors[55])); __style.text_color = __ice_palette.colors[4]; } _ => {} } if matches!(__status, ::iced::widget::button::Status::Disabled) { if let Some(::iced::Background::Color(mut __color)) = __style.background { __color.a *= 0.5; __style.background = Some(::iced::Background::Color(__color)); } __style.text_color.a *= 0.5; } __style }); ::ui_lang_runtime::accessible(__button, __a11y_id, ::ui_lang_runtime::Role::Button).logical_id_maybe(::core::cfg!(test).then_some(&*__a11y_key)).focus_id(::iced::widget::Id::from(__a11y_key)).label("Members-only posting".to_owned()).disabled(__disabled).on_activate_maybe(if __disabled { None } else { Some(__activate) }).focus_ring(__ice_palette.colors[42], 8.0).into() };
#[cfg(test)]
drop(__ice_render_source);
__ice_rendered
}); } let __child_count = __children.len(); let __children = __children.into_iter().map(|__child| ::ui_lang_runtime::bounded_fill_element(__child, __child_count, true)).collect::<::std::vec::Vec<_>>(); let __layout = ::iced::widget::row(__children).spacing(::ui_lang_runtime::bounded_spacing(8.0, __child_count)).width(::iced::Fill).align_y(::iced::alignment::Vertical::Center); let __content = ::iced::widget::container(__layout); let __layout_content: __IceElement<'_, __DucktapeMessage> = __content.into(); let __a11y_key = format!("{}/@layout:116", __ice_node_scope); ::ui_lang_runtime::accessible(__layout_content, ::ui_lang_runtime::StableId::new(&__a11y_key), ::ui_lang_runtime::Role::GenericContainer).logical_id_maybe(::core::cfg!(test).then_some(__a11y_key)).into() };
#[cfg(test)]
drop(__ice_render_source);
__ice_rendered
}); let __child_count = __children.len(); let __children = __children.into_iter().map(|__child| ::ui_lang_runtime::bounded_fill_element(__child, __child_count, false)).collect::<::std::vec::Vec<_>>(); let __layout = ::iced::widget::column(__children).spacing(::ui_lang_runtime::bounded_spacing(6.0, __child_count)).width(::iced::Fill); let __content = ::iced::widget::container(__layout); let __layout_content: __IceElement<'_, __DucktapeMessage> = __content.into(); let __a11y_key = format!("{}/@layout:111", __ice_node_scope); ::ui_lang_runtime::accessible(__layout_content, ::ui_lang_runtime::StableId::new(&__a11y_key), ::ui_lang_runtime::Role::GenericContainer).logical_id_maybe(::core::cfg!(test).then_some(__a11y_key)).into() };
#[cfg(test)]
drop(__ice_render_source);
__ice_rendered
}); __children.push({
#[cfg(test)]
let __ice_render_source_location = ::ui_lang_runtime::testing::Location::new("screens/overlays.ice", 227, 1, "rendered view node");
#[cfg(test)]
let __ice_render_source = ::ui_lang_runtime::testing::push_render_source(__ice_render_source_location);
let __ice_rendered: __IceElement<'_, __DucktapeMessage> = { let mut __children: ::std::vec::Vec<__IceElement<'_, __DucktapeMessage>> = ::std::vec::Vec::new(); __children.push({
#[cfg(test)]
let __ice_render_source_location = ::ui_lang_runtime::testing::Location::new("screens/overlays.ice", 232, 1, "rendered view node");
#[cfg(test)]
let __ice_render_source = ::ui_lang_runtime::testing::push_render_source(__ice_render_source_location);
let __ice_rendered: __IceElement<'_, __DucktapeMessage> = { let __a11y_key = format!("{}/@button:232", __ice_node_scope); let __a11y_id = ::ui_lang_runtime::StableId::new(&__a11y_key); let __disabled = (*self.__ice_derived_mutation_busy()); let __activate = (__ice_cb_6)(); let __button_content: __IceElement<'_, __DucktapeMessage> = { let __button_inner: __IceElement<'_, __DucktapeMessage> = ::iced::widget::text("Cancel").size(12.5).font(::iced::Font { weight: ::iced::font::Weight::Semibold, ..Self::default_font() }).into(); ::iced::widget::container(__button_inner).height(::iced::Fill).align_y(::iced::alignment::Vertical::Center).into() }; let __button = ::iced::widget::button(__button_content).padding(::iced::Padding { top: 11.0, right: 16.0, bottom: 11.0, left: 16.0 }).width(::iced::Fill).height(38.0 as f32).padding(9.0 as f32).on_press_maybe(if __disabled { None } else { Some(__activate.clone()) }).style(move |__theme, __status| { let mut __style = ::iced::widget::button::primary(__theme, __status); let __background: Option<::iced::Color> = match __status { ::iced::widget::button::Status::Hovered => Some(__ice_palette.colors[14]), ::iced::widget::button::Status::Pressed => Some(__ice_palette.colors[6]), ::iced::widget::button::Status::Disabled => Some(__ice_palette.colors[12]), _ => Some(__ice_palette.colors[12]) }; if let Some(__background) = __background { __style.background = Some(::iced::Background::Color(__background)); } __style.text_color = __ice_palette.colors[13]; __style.border.width = 1.0; __style.border.color = __ice_palette.colors[40]; __style.border.radius = 9.0.into(); if matches!(__status, ::iced::widget::button::Status::Disabled) { if let Some(::iced::Background::Color(mut __color)) = __style.background { __color.a *= 0.5; __style.background = Some(::iced::Background::Color(__color)); } __style.text_color.a *= 0.5; } __style }); ::ui_lang_runtime::accessible(__button, __a11y_id, ::ui_lang_runtime::Role::Button).logical_id_maybe(::core::cfg!(test).then_some(&*__a11y_key)).focus_id(::iced::widget::Id::from(__a11y_key)).label("Cancel").disabled(__disabled).on_activate_maybe(if __disabled { None } else { Some(__activate) }).focus_ring(__ice_palette.colors[42], 9.0).into() };
#[cfg(test)]
drop(__ice_render_source);
__ice_rendered
}); __children.push({
#[cfg(test)]
let __ice_render_source_location = ::ui_lang_runtime::testing::Location::new("screens/overlays.ice", 239, 1, "rendered view node");
#[cfg(test)]
let __ice_render_source = ::ui_lang_runtime::testing::push_render_source(__ice_render_source_location);
let __ice_rendered: __IceElement<'_, __DucktapeMessage> = { let __a11y_key = format!("{}/@button:239", __ice_node_scope); let __a11y_id = ::ui_lang_runtime::StableId::new(&__a11y_key); let __disabled = (((self.loading || (*self.__ice_derived_mutation_busy())) || (!self.connected)) || ((self.channel_draft).trim().to_owned()).is_empty()); let __activate = (__ice_cb_1)(); let __button_content: __IceElement<'_, __DucktapeMessage> = { let __button_inner: __IceElement<'_, __DucktapeMessage> = ::iced::widget::text("Create →").size(12.5).font(::iced::Font { weight: ::iced::font::Weight::Semibold, ..Self::default_font() }).into(); ::iced::widget::container(__button_inner).height(::iced::Fill).align_y(::iced::alignment::Vertical::Center).into() }; let __button = ::iced::widget::button(__button_content).padding(::iced::Padding { top: 11.0, right: 16.0, bottom: 11.0, left: 16.0 }).width(::iced::Fill).height(38.0 as f32).padding(9.0 as f32).on_press_maybe(if __disabled { None } else { Some(__activate.clone()) }).style(move |__theme, __status| { let mut __style = ::iced::widget::button::primary(__theme, __status); let __background: Option<::iced::Color> = match __status { ::iced::widget::button::Status::Hovered => Some(__ice_palette.colors[8]), ::iced::widget::button::Status::Pressed => Some({ let mut __color = __ice_palette.colors[7]; __color.a = 0.800000; __color }), ::iced::widget::button::Status::Disabled => Some(__ice_palette.colors[7]), _ => Some(__ice_palette.colors[7]) }; if let Some(__background) = __background { __style.background = Some(::iced::Background::Color(__background)); } __style.text_color = __ice_palette.colors[9]; __style.border.radius = 9.0.into(); if matches!(__status, ::iced::widget::button::Status::Disabled) { __style.background = Some(__ice_palette.colors[10].into()); __style.text_color = __ice_palette.colors[11]; } __style }); ::ui_lang_runtime::accessible(__button, __a11y_id, ::ui_lang_runtime::Role::Button).logical_id_maybe(::core::cfg!(test).then_some(&*__a11y_key)).focus_id(::iced::widget::Id::from(__a11y_key)).label("Create →").disabled(__disabled).on_activate_maybe(if __disabled { None } else { Some(__activate) }).focus_ring(__ice_palette.colors[42], 9.0).into() };
#[cfg(test)]
drop(__ice_render_source);
__ice_rendered
}); let __child_count = __children.len(); let __children = __children.into_iter().map(|__child| ::ui_lang_runtime::bounded_fill_element(__child, __child_count, true)).collect::<::std::vec::Vec<_>>(); let __layout = ::iced::widget::row(__children).spacing(::ui_lang_runtime::bounded_spacing(8.0, __child_count)).width(::iced::Fill).align_y(::iced::alignment::Vertical::Center); let __content = ::iced::widget::container(__layout); let __layout_content: __IceElement<'_, __DucktapeMessage> = __content.into(); let __a11y_key = format!("{}/@layout:227", __ice_node_scope); ::ui_lang_runtime::accessible(__layout_content, ::ui_lang_runtime::StableId::new(&__a11y_key), ::ui_lang_runtime::Role::GenericContainer).logical_id_maybe(::core::cfg!(test).then_some(__a11y_key)).into() };
#[cfg(test)]
drop(__ice_render_source);
__ice_rendered
}); let __child_count = __children.len(); let __children = __children.into_iter().map(|__child| ::ui_lang_runtime::bounded_fill_element(__child, __child_count, false)).collect::<::std::vec::Vec<_>>(); let __layout = ::iced::widget::column(__children).spacing(::ui_lang_runtime::bounded_spacing(13.0, __child_count)).width(::iced::Fill); let __content = ::iced::widget::container(__layout); let __layout_content: __IceElement<'_, __DucktapeMessage> = __content.into(); let __a11y_key = format!("{}/@layout:65", __ice_node_scope); ::ui_lang_runtime::accessible(__layout_content, ::ui_lang_runtime::StableId::new(&__a11y_key), ::ui_lang_runtime::Role::GenericContainer).logical_id_maybe(::core::cfg!(test).then_some(__a11y_key)).into() };
#[cfg(test)]
drop(__ice_render_source);
__ice_rendered
}; __slot_content })();
#[cfg(test)]
drop(__ice_render_source);
__ice_rendered
}); let __child_count = __children.len(); let __children = __children.into_iter().map(|__child| ::ui_lang_runtime::bounded_fill_element(__child, __child_count, false)).collect::<::std::vec::Vec<_>>(); let __layout = ::iced::widget::column(__children).spacing(::ui_lang_runtime::bounded_spacing(13.0, __child_count)).padding(::ui_lang_runtime::bounded_padding(20.0, 22.0, 22.0, 22.0)).width(::iced::Fill); let __content = ::iced::widget::container(__layout); let __layout_content: __IceElement<'_, __DucktapeMessage> = __content.into(); let __a11y_key = format!("{}/@layout:97", __ice_use_scope); ::ui_lang_runtime::accessible(__layout_content, ::ui_lang_runtime::StableId::new(&__a11y_key), ::ui_lang_runtime::Role::GenericContainer).logical_id_maybe(::core::cfg!(test).then_some(__a11y_key)).into() };
#[cfg(test)]
drop(__ice_render_source);
__ice_rendered
}; let __container = {  ::iced::widget::container(__container_content).id(::iced::widget::Id::from(__ice_node_scope.clone())).width(418.0 as f32).style(move |__theme| { let mut __style = ::iced::widget::container::Style::default(); __style.background = ::std::option::Option::Some(::iced::Background::Color(__ice_palette.colors[3])); __style.border.color = __ice_palette.colors[39]; __style.border.width = 1.0 as f32; __style.border.radius = ::iced::border::Radius { top_left: ((14.0) as f32).max(0.0).min(f32::MAX), top_right: ((14.0) as f32).max(0.0).min(f32::MAX), bottom_right: ((14.0) as f32).max(0.0).min(f32::MAX), bottom_left: ((14.0) as f32).max(0.0).min(f32::MAX) }; __style.shadow.color = __ice_palette.colors[48]; __style.shadow.offset.y = 24.0 as f32; __style.shadow.blur_radius = 60.0 as f32; __style }) }; ::ui_lang_runtime::accessible(__container, ::ui_lang_runtime::StableId::new(&__a11y_key), ::ui_lang_runtime::Role::GenericContainer).logical_id_maybe(::core::cfg!(test).then_some(__a11y_key)).into() } };
#[cfg(test)]
drop(__ice_render_source);
__ice_rendered
}; __component_content }
pub(super) fn __ice_component_use_88(&self, __ice_palette: __IcePalette, __ice_use_scope: ::std::string::String) -> __IceElement<'_, __DucktapeMessage> { let __component_content: __IceElement<'_, __DucktapeMessage> = {
#[cfg(test)]
let __ice_render_source_location = ::ui_lang_runtime::testing::Location::new("components/overlay.ice", 124, 1, "rendered view node");
#[cfg(test)]
let __ice_render_source = ::ui_lang_runtime::testing::push_render_source(__ice_render_source_location);
let __ice_rendered: __IceElement<'_, __DucktapeMessage> = { let __ice_node_scope = format!("{}/root", __ice_use_scope); { let __a11y_key = __ice_node_scope.as_str(); let __container_content: __IceElement<'_, __DucktapeMessage> = {
#[cfg(test)]
let __ice_render_source_location = ::ui_lang_runtime::testing::Location::new("components/overlay.ice", 133, 1, "rendered view node");
#[cfg(test)]
let __ice_render_source = ::ui_lang_runtime::testing::push_render_source(__ice_render_source_location);
let __ice_rendered: __IceElement<'_, __DucktapeMessage> = { let mut __children: ::std::vec::Vec<__IceElement<'_, __DucktapeMessage>> = ::std::vec::Vec::new(); if ("info" == "error") { __children.push({
#[cfg(test)]
let __ice_render_source_location = ::ui_lang_runtime::testing::Location::new("components/overlay.ice", 136, 1, "rendered view node");
#[cfg(test)]
let __ice_render_source = ::ui_lang_runtime::testing::push_render_source(__ice_render_source_location);
let __ice_rendered: __IceElement<'_, __DucktapeMessage> = { let __a11y_key = format!("{}/@container:136", __ice_use_scope); let __container_content: __IceElement<'_, __DucktapeMessage> = {
#[cfg(test)]
let __ice_render_source_location = ::ui_lang_runtime::testing::Location::new("components/overlay.ice", 142, 1, "rendered view node");
#[cfg(test)]
let __ice_render_source = ::ui_lang_runtime::testing::push_render_source(__ice_render_source_location);
let __ice_rendered: __IceElement<'_, __DucktapeMessage> = ::iced::widget::space().width(1.0 as f32).height(1.0 as f32).into();
#[cfg(test)]
drop(__ice_render_source);
__ice_rendered
}; let __container = {  ::iced::widget::container(__container_content).width(6.0 as f32).height(6.0 as f32).style(move |__theme| { let mut __style = ::iced::widget::container::Style::default(); __style.background = ::std::option::Option::Some(::iced::Background::Color(__ice_palette.colors[24])); __style.border.radius = ::iced::border::Radius { top_left: ((3.0) as f32).max(0.0).min(f32::MAX), top_right: ((3.0) as f32).max(0.0).min(f32::MAX), bottom_right: ((3.0) as f32).max(0.0).min(f32::MAX), bottom_left: ((3.0) as f32).max(0.0).min(f32::MAX) }; __style }) }; ::ui_lang_runtime::accessible(__container, ::ui_lang_runtime::StableId::new(&__a11y_key), ::ui_lang_runtime::Role::GenericContainer).logical_id_maybe(::core::cfg!(test).then_some(__a11y_key)).into() };
#[cfg(test)]
drop(__ice_render_source);
__ice_rendered
}); } if ((!("info" == "error")) && ("info" == "warning")) { __children.push({
#[cfg(test)]
let __ice_render_source_location = ::ui_lang_runtime::testing::Location::new("components/overlay.ice", 144, 1, "rendered view node");
#[cfg(test)]
let __ice_render_source = ::ui_lang_runtime::testing::push_render_source(__ice_render_source_location);
let __ice_rendered: __IceElement<'_, __DucktapeMessage> = { let __a11y_key = format!("{}/@container:144", __ice_use_scope); let __container_content: __IceElement<'_, __DucktapeMessage> = {
#[cfg(test)]
let __ice_render_source_location = ::ui_lang_runtime::testing::Location::new("components/overlay.ice", 150, 1, "rendered view node");
#[cfg(test)]
let __ice_render_source = ::ui_lang_runtime::testing::push_render_source(__ice_render_source_location);
let __ice_rendered: __IceElement<'_, __DucktapeMessage> = ::iced::widget::space().width(1.0 as f32).height(1.0 as f32).into();
#[cfg(test)]
drop(__ice_render_source);
__ice_rendered
}; let __container = {  ::iced::widget::container(__container_content).width(6.0 as f32).height(6.0 as f32).style(move |__theme| { let mut __style = ::iced::widget::container::Style::default(); __style.background = ::std::option::Option::Some(::iced::Background::Color(__ice_palette.colors[34])); __style.border.radius = ::iced::border::Radius { top_left: ((3.0) as f32).max(0.0).min(f32::MAX), top_right: ((3.0) as f32).max(0.0).min(f32::MAX), bottom_right: ((3.0) as f32).max(0.0).min(f32::MAX), bottom_left: ((3.0) as f32).max(0.0).min(f32::MAX) }; __style }) }; ::ui_lang_runtime::accessible(__container, ::ui_lang_runtime::StableId::new(&__a11y_key), ::ui_lang_runtime::Role::GenericContainer).logical_id_maybe(::core::cfg!(test).then_some(__a11y_key)).into() };
#[cfg(test)]
drop(__ice_render_source);
__ice_rendered
}); } if (!(("info" == "error") || ("info" == "warning"))) { __children.push({
#[cfg(test)]
let __ice_render_source_location = ::ui_lang_runtime::testing::Location::new("components/overlay.ice", 152, 1, "rendered view node");
#[cfg(test)]
let __ice_render_source = ::ui_lang_runtime::testing::push_render_source(__ice_render_source_location);
let __ice_rendered: __IceElement<'_, __DucktapeMessage> = { let __a11y_key = format!("{}/@container:152", __ice_use_scope); let __container_content: __IceElement<'_, __DucktapeMessage> = {
#[cfg(test)]
let __ice_render_source_location = ::ui_lang_runtime::testing::Location::new("components/overlay.ice", 158, 1, "rendered view node");
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
}); } __children.push({
#[cfg(test)]
let __ice_render_source_location = ::ui_lang_runtime::testing::Location::new("components/overlay.ice", 159, 1, "rendered view node");
#[cfg(test)]
let __ice_render_source = ::ui_lang_runtime::testing::push_render_source(__ice_render_source_location);
let __ice_rendered: __IceElement<'_, __DucktapeMessage> = { let __a11y_key = format!("{}/@text:159", __ice_use_scope); let __text_value = (self.toast).to_string(); let __text = ::iced::widget::text(__text_value.clone()).size(((12.5) as f32).max(f32::EPSILON).min(f32::MAX)).wrapping(::iced::widget::text::Wrapping::None).color(__ice_palette.colors[38]); let __text = ::ui_lang_runtime::selectable_text(__text); ::ui_lang_runtime::accessible(__text, ::ui_lang_runtime::StableId::new(&__a11y_key), ::ui_lang_runtime::Role::Label).logical_id_maybe(::core::cfg!(test).then_some(__a11y_key)).value(__text_value).into() };
#[cfg(test)]
drop(__ice_render_source);
__ice_rendered
}); let __child_count = __children.len(); let __children = __children.into_iter().map(|__child| ::ui_lang_runtime::bounded_fill_element(__child, __child_count, true)).collect::<::std::vec::Vec<_>>(); let __layout = ::iced::widget::row(__children).spacing(::ui_lang_runtime::bounded_spacing(9.0, __child_count)).align_y(::iced::alignment::Vertical::Center); let __content = ::iced::widget::container(__layout); let __layout_content: __IceElement<'_, __DucktapeMessage> = __content.into(); let __a11y_key = format!("{}/@layout:133", __ice_use_scope); ::ui_lang_runtime::accessible(__layout_content, ::ui_lang_runtime::StableId::new(&__a11y_key), ::ui_lang_runtime::Role::GenericContainer).logical_id_maybe(::core::cfg!(test).then_some(__a11y_key)).into() };
#[cfg(test)]
drop(__ice_render_source);
__ice_rendered
}; let __container = {  ::iced::widget::container(__container_content).id(::iced::widget::Id::from(__ice_node_scope.clone())).padding(::ui_lang_runtime::bounded_padding(10.0, 16.0, 10.0, 16.0)).style(move |__theme| { let mut __style = ::iced::widget::container::Style::default(); __style.background = ::std::option::Option::Some(::iced::Background::Color(__ice_palette.colors[37])); __style.border.radius = ::iced::border::Radius { top_left: ((10.0) as f32).max(0.0).min(f32::MAX), top_right: ((10.0) as f32).max(0.0).min(f32::MAX), bottom_right: ((10.0) as f32).max(0.0).min(f32::MAX), bottom_left: ((10.0) as f32).max(0.0).min(f32::MAX) }; __style.shadow.color = __ice_palette.colors[47]; __style.shadow.offset.y = 6.0 as f32; __style.shadow.blur_radius = 18.0 as f32; __style }) }; ::ui_lang_runtime::accessible(__container, ::ui_lang_runtime::StableId::new(&__a11y_key), ::ui_lang_runtime::Role::GenericContainer).logical_id_maybe(::core::cfg!(test).then_some(__a11y_key)).into() } };
#[cfg(test)]
drop(__ice_render_source);
__ice_rendered
}; __component_content }
#[cfg(test)]
pub(super) fn __ice_component_use_182(&self, __ice_palette: __IcePalette, __ice_use_scope: ::std::string::String) -> __IceElement<'_, __DucktapeMessage> { let __component_content: __IceElement<'_, __DucktapeMessage> = {
#[cfg(test)]
let __ice_render_source_location = ::ui_lang_runtime::testing::Location::new("components/overlay.ice", 253, 1, "rendered view node");
#[cfg(test)]
let __ice_render_source = ::ui_lang_runtime::testing::push_render_source(__ice_render_source_location);
let __ice_rendered: __IceElement<'_, __DucktapeMessage> = { let __ice_node_scope = format!("{}/root", __ice_use_scope); { let mut __children: ::std::vec::Vec<__IceElement<'_, __DucktapeMessage>> = ::std::vec::Vec::new(); __children.push({
#[cfg(test)]
let __ice_render_source_location = ::ui_lang_runtime::testing::Location::new("components/overlay.ice", 254, 1, "rendered view node");
#[cfg(test)]
let __ice_render_source = ::ui_lang_runtime::testing::push_render_source(__ice_render_source_location);
let __ice_rendered: __IceElement<'_, __DucktapeMessage> = { let __a11y_key = format!("{}/@text:254", __ice_use_scope); let __text_value = ("CHANNEL NAME").to_string(); let __text = ::iced::widget::text(__text_value.clone()).size(((9.0) as f32).max(f32::EPSILON).min(f32::MAX)).wrapping(::iced::widget::text::Wrapping::None).font(::iced::Font { family: ::iced::font::Family::Name("Geist Mono"), weight: ::iced::font::Weight::Semibold, stretch: ::iced::font::Stretch::Normal, style: ::iced::font::Style::Normal }).color(__ice_palette.colors[73]); let __text = ::ui_lang_runtime::selectable_text(__text); ::ui_lang_runtime::accessible(__text, ::ui_lang_runtime::StableId::new(&__a11y_key), ::ui_lang_runtime::Role::Label).logical_id_maybe(::core::cfg!(test).then_some(__a11y_key)).value(__text_value).into() };
#[cfg(test)]
drop(__ice_render_source);
__ice_rendered
}); if ("" != "") { __children.push({
#[cfg(test)]
let __ice_render_source_location = ::ui_lang_runtime::testing::Location::new("components/overlay.ice", 261, 1, "rendered view node");
#[cfg(test)]
let __ice_render_source = ::ui_lang_runtime::testing::push_render_source(__ice_render_source_location);
let __ice_rendered: __IceElement<'_, __DucktapeMessage> = { let __a11y_key = format!("{}/@text:261", __ice_use_scope); let __text_value = ("").to_string(); let __text = ::iced::widget::text(__text_value.clone()).size(((9.0) as f32).max(f32::EPSILON).min(f32::MAX)).wrapping(::iced::widget::text::Wrapping::None).font(::iced::Font { family: ::iced::font::Family::Name("Geist Mono"), weight: ::iced::font::Weight::Semibold, stretch: ::iced::font::Stretch::Normal, style: ::iced::font::Style::Normal }).color(__ice_palette.colors[73]); let __text = ::ui_lang_runtime::selectable_text(__text); ::ui_lang_runtime::accessible(__text, ::ui_lang_runtime::StableId::new(&__a11y_key), ::ui_lang_runtime::Role::Label).logical_id_maybe(::core::cfg!(test).then_some(__a11y_key)).value(__text_value).into() };
#[cfg(test)]
drop(__ice_render_source);
__ice_rendered
}); } let __child_count = __children.len(); let __children = __children.into_iter().map(|__child| ::ui_lang_runtime::bounded_fill_element(__child, __child_count, true)).collect::<::std::vec::Vec<_>>(); let __layout = ::iced::widget::row(__children).spacing(::ui_lang_runtime::bounded_spacing(8.0, __child_count)).align_y(::iced::alignment::Vertical::Center); let __content = ::iced::widget::container(__layout); let __layout_content: __IceElement<'_, __DucktapeMessage> = __content.into(); let __a11y_key = __ice_node_scope.as_str(); ::ui_lang_runtime::accessible(__layout_content, ::ui_lang_runtime::StableId::new(&__a11y_key), ::ui_lang_runtime::Role::GenericContainer).logical_id_maybe(::core::cfg!(test).then_some(__a11y_key)).into() } };
#[cfg(test)]
drop(__ice_render_source);
__ice_rendered
}; __component_content }
#[cfg(test)]
pub(super) fn __ice_component_use_183(&self, __ice_palette: __IcePalette, __ice_use_scope: ::std::string::String) -> __IceElement<'_, __DucktapeMessage> { let __component_content: __IceElement<'_, __DucktapeMessage> = {
#[cfg(test)]
let __ice_render_source_location = ::ui_lang_runtime::testing::Location::new("components/overlay.ice", 253, 1, "rendered view node");
#[cfg(test)]
let __ice_render_source = ::ui_lang_runtime::testing::push_render_source(__ice_render_source_location);
let __ice_rendered: __IceElement<'_, __DucktapeMessage> = { let __ice_node_scope = format!("{}/root", __ice_use_scope); { let mut __children: ::std::vec::Vec<__IceElement<'_, __DucktapeMessage>> = ::std::vec::Vec::new(); __children.push({
#[cfg(test)]
let __ice_render_source_location = ::ui_lang_runtime::testing::Location::new("components/overlay.ice", 254, 1, "rendered view node");
#[cfg(test)]
let __ice_render_source = ::ui_lang_runtime::testing::push_render_source(__ice_render_source_location);
let __ice_rendered: __IceElement<'_, __DucktapeMessage> = { let __a11y_key = format!("{}/@text:254", __ice_use_scope); let __text_value = ("POSTING").to_string(); let __text = ::iced::widget::text(__text_value.clone()).size(((9.0) as f32).max(f32::EPSILON).min(f32::MAX)).wrapping(::iced::widget::text::Wrapping::None).font(::iced::Font { family: ::iced::font::Family::Name("Geist Mono"), weight: ::iced::font::Weight::Semibold, stretch: ::iced::font::Stretch::Normal, style: ::iced::font::Style::Normal }).color(__ice_palette.colors[73]); let __text = ::ui_lang_runtime::selectable_text(__text); ::ui_lang_runtime::accessible(__text, ::ui_lang_runtime::StableId::new(&__a11y_key), ::ui_lang_runtime::Role::Label).logical_id_maybe(::core::cfg!(test).then_some(__a11y_key)).value(__text_value).into() };
#[cfg(test)]
drop(__ice_render_source);
__ice_rendered
}); if ("" != "") { __children.push({
#[cfg(test)]
let __ice_render_source_location = ::ui_lang_runtime::testing::Location::new("components/overlay.ice", 261, 1, "rendered view node");
#[cfg(test)]
let __ice_render_source = ::ui_lang_runtime::testing::push_render_source(__ice_render_source_location);
let __ice_rendered: __IceElement<'_, __DucktapeMessage> = { let __a11y_key = format!("{}/@text:261", __ice_use_scope); let __text_value = ("").to_string(); let __text = ::iced::widget::text(__text_value.clone()).size(((9.0) as f32).max(f32::EPSILON).min(f32::MAX)).wrapping(::iced::widget::text::Wrapping::None).font(::iced::Font { family: ::iced::font::Family::Name("Geist Mono"), weight: ::iced::font::Weight::Semibold, stretch: ::iced::font::Stretch::Normal, style: ::iced::font::Style::Normal }).color(__ice_palette.colors[73]); let __text = ::ui_lang_runtime::selectable_text(__text); ::ui_lang_runtime::accessible(__text, ::ui_lang_runtime::StableId::new(&__a11y_key), ::ui_lang_runtime::Role::Label).logical_id_maybe(::core::cfg!(test).then_some(__a11y_key)).value(__text_value).into() };
#[cfg(test)]
drop(__ice_render_source);
__ice_rendered
}); } let __child_count = __children.len(); let __children = __children.into_iter().map(|__child| ::ui_lang_runtime::bounded_fill_element(__child, __child_count, true)).collect::<::std::vec::Vec<_>>(); let __layout = ::iced::widget::row(__children).spacing(::ui_lang_runtime::bounded_spacing(8.0, __child_count)).align_y(::iced::alignment::Vertical::Center); let __content = ::iced::widget::container(__layout); let __layout_content: __IceElement<'_, __DucktapeMessage> = __content.into(); let __a11y_key = __ice_node_scope.as_str(); ::ui_lang_runtime::accessible(__layout_content, ::ui_lang_runtime::StableId::new(&__a11y_key), ::ui_lang_runtime::Role::GenericContainer).logical_id_maybe(::core::cfg!(test).then_some(__a11y_key)).into() } };
#[cfg(test)]
drop(__ice_render_source);
__ice_rendered
}; __component_content }
#[cfg(test)]
pub(super) fn __ice_component_use_184(&self, __ice_palette: __IcePalette, __ice_use_scope: ::std::string::String, __ice_cb_0: impl Fn() -> __DucktapeMessage + Clone + 'static, __ice_cb_1: impl Fn() -> __DucktapeMessage + Clone + 'static, __ice_cb_2: impl Fn() -> __DucktapeMessage + Clone + 'static, __ice_cb_3: impl Fn(::std::string::String, i64) -> __DucktapeMessage + Clone + 'static, __ice_cb_4: impl Fn(::std::string::String, ::std::string::String) -> __DucktapeMessage + Clone + 'static, __ice_cb_5: impl Fn(::std::string::String) -> __DucktapeMessage + Clone + 'static, __ice_cb_6: impl Fn() -> __DucktapeMessage + Clone + 'static, __ice_cb_7: impl Fn() -> __DucktapeMessage + Clone + 'static) -> __IceElement<'_, __DucktapeMessage> { let __component_content: __IceElement<'_, __DucktapeMessage> = {
#[cfg(test)]
let __ice_render_source_location = ::ui_lang_runtime::testing::Location::new("components/overlay.ice", 87, 1, "rendered view node");
#[cfg(test)]
let __ice_render_source = ::ui_lang_runtime::testing::push_render_source(__ice_render_source_location);
let __ice_rendered: __IceElement<'_, __DucktapeMessage> = { let __ice_node_scope = format!("{}/root", __ice_use_scope); { let __a11y_key = __ice_node_scope.as_str(); let __container_content: __IceElement<'_, __DucktapeMessage> = {
#[cfg(test)]
let __ice_render_source_location = ::ui_lang_runtime::testing::Location::new("components/overlay.ice", 97, 1, "rendered view node");
#[cfg(test)]
let __ice_render_source = ::ui_lang_runtime::testing::push_render_source(__ice_render_source_location);
let __ice_rendered: __IceElement<'_, __DucktapeMessage> = { let mut __children: ::std::vec::Vec<__IceElement<'_, __DucktapeMessage>> = ::std::vec::Vec::new(); __children.push({
#[cfg(test)]
let __ice_render_source_location = ::ui_lang_runtime::testing::Location::new("components/overlay.ice", 105, 1, "rendered view node");
#[cfg(test)]
let __ice_render_source = ::ui_lang_runtime::testing::push_render_source(__ice_render_source_location);
let __ice_rendered: __IceElement<'_, __DucktapeMessage> = { let mut __children: ::std::vec::Vec<__IceElement<'_, __DucktapeMessage>> = ::std::vec::Vec::new(); __children.push({
#[cfg(test)]
let __ice_render_source_location = ::ui_lang_runtime::testing::Location::new("components/overlay.ice", 110, 1, "rendered view node");
#[cfg(test)]
let __ice_render_source = ::ui_lang_runtime::testing::push_render_source(__ice_render_source_location);
let __ice_rendered: __IceElement<'_, __DucktapeMessage> = { let __a11y_key = format!("{}/@text:110", __ice_use_scope); let __text_value = ("Create a channel").to_string(); let __text = ::iced::widget::text(__text_value.clone()).size(((16.0) as f32).max(f32::EPSILON).min(f32::MAX)).width(::iced::Fill).wrapping(::iced::widget::text::Wrapping::None).font(::iced::Font { family: ::iced::font::Family::Name("Geist"), weight: ::iced::font::Weight::Semibold, stretch: ::iced::font::Stretch::Normal, style: ::iced::font::Style::Normal }).color(__ice_palette.colors[7]); let __text = ::ui_lang_runtime::selectable_text(__text); ::ui_lang_runtime::accessible(__text, ::ui_lang_runtime::StableId::new(&__a11y_key), ::ui_lang_runtime::Role::Label).logical_id_maybe(::core::cfg!(test).then_some(__a11y_key)).value(__text_value).into() };
#[cfg(test)]
drop(__ice_render_source);
__ice_rendered
}); __children.push({
#[cfg(test)]
let __ice_render_source_location = ::ui_lang_runtime::testing::Location::new("components/overlay.ice", 117, 1, "rendered view node");
#[cfg(test)]
let __ice_render_source = ::ui_lang_runtime::testing::push_render_source(__ice_render_source_location);
let __ice_rendered: __IceElement<'_, __DucktapeMessage> = (|| { let __slot_content: __IceElement<'_, __DucktapeMessage> = {
#[cfg(test)]
let __ice_render_source_location = ::ui_lang_runtime::testing::Location::new("screens/overlays.ice", 42, 1, "rendered view node");
#[cfg(test)]
let __ice_render_source = ::ui_lang_runtime::testing::push_render_source(__ice_render_source_location);
let __ice_rendered: __IceElement<'_, __DucktapeMessage> = { let __a11y_key = format!("{}/@button:42", __ice_node_scope); let __a11y_id = ::ui_lang_runtime::StableId::new(&__a11y_key); let __disabled = false; let __activate = (__ice_cb_6)(); let __button_content: __IceElement<'_, __DucktapeMessage> = { let __button_inner: __IceElement<'_, __DucktapeMessage> = {
#[cfg(test)]
let __ice_render_source_location = ::ui_lang_runtime::testing::Location::new("screens/overlays.ice", 50, 1, "rendered view node");
#[cfg(test)]
let __ice_render_source = ::ui_lang_runtime::testing::push_render_source(__ice_render_source_location);
let __ice_rendered: __IceElement<'_, __DucktapeMessage> = { let __a11y_key = format!("{}/@container:50", __ice_node_scope); let __container_content: __IceElement<'_, __DucktapeMessage> = {
#[cfg(test)]
let __ice_render_source_location = ::ui_lang_runtime::testing::Location::new("screens/overlays.ice", 56, 1, "rendered view node");
#[cfg(test)]
let __ice_render_source = ::ui_lang_runtime::testing::push_render_source(__ice_render_source_location);
let __ice_rendered: __IceElement<'_, __DucktapeMessage> = { let __a11y_key = format!("{}/@text:56", __ice_node_scope); let __text_value = ("×").to_string(); let __text = ::iced::widget::text(__text_value.clone()).size(((14.0) as f32).max(f32::EPSILON).min(f32::MAX)).wrapping(::iced::widget::text::Wrapping::None).color(__ice_palette.colors[5]); let __text = ::ui_lang_runtime::selectable_text(__text); ::ui_lang_runtime::accessible(__text, ::ui_lang_runtime::StableId::new(&__a11y_key), ::ui_lang_runtime::Role::Label).logical_id_maybe(::core::cfg!(test).then_some(__a11y_key)).value(__text_value).into() };
#[cfg(test)]
drop(__ice_render_source);
__ice_rendered
}; let __container = {  ::iced::widget::container(__container_content).width(::iced::Fill).height(::iced::Fill).align_x(::iced::alignment::Horizontal::Center).align_y(::iced::alignment::Vertical::Center) }; ::ui_lang_runtime::accessible(__container, ::ui_lang_runtime::StableId::new(&__a11y_key), ::ui_lang_runtime::Role::GenericContainer).logical_id_maybe(::core::cfg!(test).then_some(__a11y_key)).into() };
#[cfg(test)]
drop(__ice_render_source);
__ice_rendered
}; ::iced::widget::container(__button_inner).width(::iced::Fill).align_x(::iced::alignment::Horizontal::Center).height(::iced::Fill).align_y(::iced::alignment::Vertical::Center).into() }; let __button = ::iced::widget::button(__button_content).width(26.0 as f32).height(26.0 as f32).padding(0.0 as f32).on_press_maybe(if __disabled { None } else { Some(__activate.clone()) }).style(move |__theme, __status| { let mut __style = ::iced::widget::button::primary(__theme, __status); let __background: Option<::iced::Color> = match __status { ::iced::widget::button::Status::Hovered => Some(__ice_palette.colors[14]), ::iced::widget::button::Status::Pressed => Some(__ice_palette.colors[39]), ::iced::widget::button::Status::Disabled => Some(::iced::Color::from_rgba8(0, 0, 0, 0.000000)), _ => Some(::iced::Color::from_rgba8(0, 0, 0, 0.000000)) }; if let Some(__background) = __background { __style.background = Some(::iced::Background::Color(__background)); } __style.text_color = __ice_palette.colors[4]; __style.border.radius = 7.0.into(); __style.background = ::std::option::Option::Some(::iced::Background::Color(::iced::Color::from_rgba8(0, 0, 0, 0.000000))); __style.text_color = __ice_palette.colors[5]; __style.border.color = ::iced::Color::from_rgba8(0, 0, 0, 0.000000); __style.border.width = 1.0 as f32; __style.border.radius = ::iced::border::Radius { top_left: ((7.0) as f32).max(0.0).min(f32::MAX), top_right: ((7.0) as f32).max(0.0).min(f32::MAX), bottom_right: ((7.0) as f32).max(0.0).min(f32::MAX), bottom_left: ((7.0) as f32).max(0.0).min(f32::MAX) }; match __status { ::iced::widget::button::Status::Hovered => { __style.background = ::std::option::Option::Some(::iced::Background::Color(__ice_palette.colors[55])); __style.text_color = __ice_palette.colors[4]; } ::iced::widget::button::Status::Pressed => { __style.background = ::std::option::Option::Some(::iced::Background::Color(__ice_palette.colors[56])); __style.text_color = __ice_palette.colors[4]; } _ => {} } if matches!(__status, ::iced::widget::button::Status::Disabled) { if let Some(::iced::Background::Color(mut __color)) = __style.background { __color.a *= 0.5; __style.background = Some(::iced::Background::Color(__color)); } __style.text_color.a *= 0.5; } __style }); ::ui_lang_runtime::accessible(__button, __a11y_id, ::ui_lang_runtime::Role::Button).logical_id_maybe(::core::cfg!(test).then_some(&*__a11y_key)).focus_id(::iced::widget::Id::from(__a11y_key)).label("Close".to_owned()).disabled(__disabled).on_activate_maybe(if __disabled { None } else { Some(__activate) }).focus_ring(__ice_palette.colors[42], 7.0).into() };
#[cfg(test)]
drop(__ice_render_source);
__ice_rendered
}; __slot_content })();
#[cfg(test)]
drop(__ice_render_source);
__ice_rendered
}); let __child_count = __children.len(); let __children = __children.into_iter().map(|__child| ::ui_lang_runtime::bounded_fill_element(__child, __child_count, true)).collect::<::std::vec::Vec<_>>(); let __layout = ::iced::widget::row(__children).spacing(::ui_lang_runtime::bounded_spacing(10.0, __child_count)).width(::iced::Fill).align_y(::iced::alignment::Vertical::Center); let __content = ::iced::widget::container(__layout); let __layout_content: __IceElement<'_, __DucktapeMessage> = __content.into(); let __a11y_key = format!("{}/@layout:105", __ice_use_scope); ::ui_lang_runtime::accessible(__layout_content, ::ui_lang_runtime::StableId::new(&__a11y_key), ::ui_lang_runtime::Role::GenericContainer).logical_id_maybe(::core::cfg!(test).then_some(__a11y_key)).into() };
#[cfg(test)]
drop(__ice_render_source);
__ice_rendered
}); __children.push({
#[cfg(test)]
let __ice_render_source_location = ::ui_lang_runtime::testing::Location::new("components/overlay.ice", 118, 1, "rendered view node");
#[cfg(test)]
let __ice_render_source = ::ui_lang_runtime::testing::push_render_source(__ice_render_source_location);
let __ice_rendered: __IceElement<'_, __DucktapeMessage> = (|| { let __slot_content: __IceElement<'_, __DucktapeMessage> = {
#[cfg(test)]
let __ice_render_source_location = ::ui_lang_runtime::testing::Location::new("screens/overlays.ice", 65, 1, "rendered view node");
#[cfg(test)]
let __ice_render_source = ::ui_lang_runtime::testing::push_render_source(__ice_render_source_location);
let __ice_rendered: __IceElement<'_, __DucktapeMessage> = { let mut __children: ::std::vec::Vec<__IceElement<'_, __DucktapeMessage>> = ::std::vec::Vec::new(); __children.push({
#[cfg(test)]
let __ice_render_source_location = ::ui_lang_runtime::testing::Location::new("screens/overlays.ice", 66, 1, "rendered view node");
#[cfg(test)]
let __ice_render_source = ::ui_lang_runtime::testing::push_render_source(__ice_render_source_location);
let __ice_rendered: __IceElement<'_, __DucktapeMessage> = { let __a11y_key = format!("{}/@text:66", __ice_node_scope); let __text_value = ("The channel is created immediately — there is no proposal and no approval step.").to_string(); let __text = ::iced::widget::text(__text_value.clone()).size(((12.0) as f32).max(f32::EPSILON).min(f32::MAX)).width(::iced::Fill).line_height(::iced::widget::text::LineHeight::Relative(((1.5) as f32).max(f32::EPSILON).min(f32::MAX))).color(__ice_palette.colors[70]); let __text = ::ui_lang_runtime::selectable_text(__text); ::ui_lang_runtime::accessible(__text, ::ui_lang_runtime::StableId::new(&__a11y_key), ::ui_lang_runtime::Role::Label).logical_id_maybe(::core::cfg!(test).then_some(__a11y_key)).value(__text_value).into() };
#[cfg(test)]
drop(__ice_render_source);
__ice_rendered
}); __children.push({
#[cfg(test)]
let __ice_render_source_location = ::ui_lang_runtime::testing::Location::new("screens/overlays.ice", 72, 1, "rendered view node");
#[cfg(test)]
let __ice_render_source = ::ui_lang_runtime::testing::push_render_source(__ice_render_source_location);
let __ice_rendered: __IceElement<'_, __DucktapeMessage> = { let mut __children: ::std::vec::Vec<__IceElement<'_, __DucktapeMessage>> = ::std::vec::Vec::new(); __children.push({
#[cfg(test)]
let __ice_render_source_location = ::ui_lang_runtime::testing::Location::new("screens/overlays.ice", 73, 1, "rendered view node");
#[cfg(test)]
let __ice_render_source = ::ui_lang_runtime::testing::push_render_source(__ice_render_source_location);
let __ice_rendered: __IceElement<'_, __DucktapeMessage> = { let __ice_component_eyebrow_scope_6633 = format!("{}/Eyebrow@6633", __ice_node_scope); ::ui_lang_runtime::rev_memo(1028u64, (__ice_palette.name), ::ui_lang_runtime::grow_stack(|| self.__ice_component_use_182(__ice_palette, __ice_component_eyebrow_scope_6633))).into() };
#[cfg(test)]
drop(__ice_render_source);
__ice_rendered
}); __children.push({
#[cfg(test)]
let __ice_render_source_location = ::ui_lang_runtime::testing::Location::new("screens/overlays.ice", 74, 1, "rendered view node");
#[cfg(test)]
let __ice_render_source = ::ui_lang_runtime::testing::push_render_source(__ice_render_source_location);
let __ice_rendered: __IceElement<'_, __DucktapeMessage> = { let __a11y_key = format!("{}/@container:74", __ice_node_scope); let __container_content: __IceElement<'_, __DucktapeMessage> = {
#[cfg(test)]
let __ice_render_source_location = ::ui_lang_runtime::testing::Location::new("screens/overlays.ice", 85, 1, "rendered view node");
#[cfg(test)]
let __ice_render_source = ::ui_lang_runtime::testing::push_render_source(__ice_render_source_location);
let __ice_rendered: __IceElement<'_, __DucktapeMessage> = { let mut __children: ::std::vec::Vec<__IceElement<'_, __DucktapeMessage>> = ::std::vec::Vec::new(); __children.push({
#[cfg(test)]
let __ice_render_source_location = ::ui_lang_runtime::testing::Location::new("screens/overlays.ice", 90, 1, "rendered view node");
#[cfg(test)]
let __ice_render_source = ::ui_lang_runtime::testing::push_render_source(__ice_render_source_location);
let __ice_rendered: __IceElement<'_, __DucktapeMessage> = { let __a11y_key = format!("{}/@text:90", __ice_node_scope); let __text_value = ("#").to_string(); let __text = ::iced::widget::text(__text_value.clone()).size(((14.0) as f32).max(f32::EPSILON).min(f32::MAX)).wrapping(::iced::widget::text::Wrapping::None).font(::iced::Font { family: ::iced::font::Family::Name("Geist Mono"), weight: ::iced::font::Weight::Medium, stretch: ::iced::font::Stretch::Normal, style: ::iced::font::Style::Normal }).color(__ice_palette.colors[73]); let __text = ::ui_lang_runtime::selectable_text(__text); ::ui_lang_runtime::accessible(__text, ::ui_lang_runtime::StableId::new(&__a11y_key), ::ui_lang_runtime::Role::Label).logical_id_maybe(::core::cfg!(test).then_some(__a11y_key)).value(__text_value).into() };
#[cfg(test)]
drop(__ice_render_source);
__ice_rendered
}); __children.push({
#[cfg(test)]
let __ice_render_source_location = ::ui_lang_runtime::testing::Location::new("screens/overlays.ice", 96, 1, "rendered view node");
#[cfg(test)]
let __ice_render_source = ::ui_lang_runtime::testing::push_render_source(__ice_render_source_location);
let __ice_rendered: __IceElement<'_, __DucktapeMessage> = { let __ice_node_scope = format!("{}/new-channel", __ice_node_scope); { let __a11y_key = __ice_node_scope.clone(); let __a11y_id = ::ui_lang_runtime::StableId::new(&__a11y_key); let __disabled = ((false || false) || (!true)); let __secure = false; let __role = if __secure { ::ui_lang_runtime::Role::PasswordInput } else { ::ui_lang_runtime::Role::TextInput }; let __input = ::ui_lang_runtime::accessible(::iced::widget::text_input("design-review", &self.channel_draft).id(::iced::widget::Id::from(__a11y_key.clone())).padding(::iced::Padding { top: 11.0, right: 13.0, bottom: 11.0, left: 13.0 }).width(::iced::Fill).secure(__secure).width(::iced::Fill).padding(6.2 as f32).size(((13.0) as f32).max(f32::EPSILON).min(f32::MAX)).line_height(::iced::widget::text::LineHeight::Relative(((1.2) as f32).max(f32::EPSILON).min(f32::MAX))).font(::iced::Font { family: ::iced::font::Family::Name("Geist Mono"), weight: ::iced::font::Weight::Normal, stretch: ::iced::font::Stretch::Normal, style: ::iced::font::Style::Normal }).on_input_maybe(if __disabled { None } else { Some(__DucktapeMessage::__BindChannelDraft as fn(::std::string::String) -> __DucktapeMessage) }).on_submit_maybe(if __disabled { None } else { Some((__ice_cb_1)()) }).style(move |__theme, __status| { let mut __style = ::iced::widget::text_input::default(__theme, __status); __style.background = __ice_palette.colors[3].into(); __style.border.color = __ice_palette.colors[39]; __style.border.width = 1.0; __style.border.radius = 10.0.into(); __style.background = ::iced::Background::Color(::iced::Color::from_rgba8(0, 0, 0, 0.000000)); __style.border.color = ::iced::Color::from_rgba8(0, 0, 0, 0.000000); __style.border.width = 0.0 as f32; __style.border.radius = ::iced::border::Radius { top_left: ((0.0) as f32).max(0.0).min(f32::MAX), top_right: ((0.0) as f32).max(0.0).min(f32::MAX), bottom_right: ((0.0) as f32).max(0.0).min(f32::MAX), bottom_left: ((0.0) as f32).max(0.0).min(f32::MAX) }; __style.placeholder = __ice_palette.colors[72]; __style.value = __ice_palette.colors[4]; __style.selection = { let mut __color = __ice_palette.colors[4]; __color.a = 0.180000; __color }; if matches!(__status, ::iced::widget::text_input::Status::Focused { .. }) { __style.border.color = __ice_palette.colors[42]; } match __status { ::iced::widget::text_input::Status::Hovered => { __style.background = ::iced::Background::Color(::iced::Color::from_rgba8(0, 0, 0, 0.000000)); __style.border.color = ::iced::Color::from_rgba8(0, 0, 0, 0.000000); } ::iced::widget::text_input::Status::Disabled => { __style.value = __ice_palette.colors[5]; } _ => {} } __style }), __a11y_id, __role).logical_id_maybe(::core::cfg!(test).then_some(&*__a11y_key)).focus_id(::iced::widget::Id::from(__a11y_key)).label("New channel name".to_owned()).value_maybe((!__secure).then(|| (self.channel_draft).to_owned())).disabled(__disabled); __input.into() } };
#[cfg(test)]
drop(__ice_render_source);
__ice_rendered
}); let __child_count = __children.len(); let __children = __children.into_iter().map(|__child| ::ui_lang_runtime::bounded_fill_element(__child, __child_count, true)).collect::<::std::vec::Vec<_>>(); let __layout = ::iced::widget::row(__children).spacing(::ui_lang_runtime::bounded_spacing(7.0, __child_count)).width(::iced::Fill).align_y(::iced::alignment::Vertical::Center); let __content = ::iced::widget::container(__layout); let __layout_content: __IceElement<'_, __DucktapeMessage> = __content.into(); let __a11y_key = format!("{}/@layout:85", __ice_node_scope); ::ui_lang_runtime::accessible(__layout_content, ::ui_lang_runtime::StableId::new(&__a11y_key), ::ui_lang_runtime::Role::GenericContainer).logical_id_maybe(::core::cfg!(test).then_some(__a11y_key)).into() };
#[cfg(test)]
drop(__ice_render_source);
__ice_rendered
}; let __container = {  ::iced::widget::container(__container_content).padding(::ui_lang_runtime::bounded_padding(2.0, 11.0, 2.0, 11.0)).width(::iced::Fill).style(move |__theme| { let mut __style = ::iced::widget::container::Style::default(); __style.background = ::std::option::Option::Some(::iced::Background::Color(__ice_palette.colors[3])); __style.border.color = __ice_palette.colors[7]; __style.border.width = 1.5 as f32; __style.border.radius = ::iced::border::Radius { top_left: ((9.0) as f32).max(0.0).min(f32::MAX), top_right: ((9.0) as f32).max(0.0).min(f32::MAX), bottom_right: ((9.0) as f32).max(0.0).min(f32::MAX), bottom_left: ((9.0) as f32).max(0.0).min(f32::MAX) }; __style }) }; ::ui_lang_runtime::accessible(__container, ::ui_lang_runtime::StableId::new(&__a11y_key), ::ui_lang_runtime::Role::GenericContainer).logical_id_maybe(::core::cfg!(test).then_some(__a11y_key)).into() };
#[cfg(test)]
drop(__ice_render_source);
__ice_rendered
}); let __child_count = __children.len(); let __children = __children.into_iter().map(|__child| ::ui_lang_runtime::bounded_fill_element(__child, __child_count, false)).collect::<::std::vec::Vec<_>>(); let __layout = ::iced::widget::column(__children).spacing(::ui_lang_runtime::bounded_spacing(6.0, __child_count)).width(::iced::Fill); let __content = ::iced::widget::container(__layout); let __layout_content: __IceElement<'_, __DucktapeMessage> = __content.into(); let __a11y_key = format!("{}/@layout:72", __ice_node_scope); ::ui_lang_runtime::accessible(__layout_content, ::ui_lang_runtime::StableId::new(&__a11y_key), ::ui_lang_runtime::Role::GenericContainer).logical_id_maybe(::core::cfg!(test).then_some(__a11y_key)).into() };
#[cfg(test)]
drop(__ice_render_source);
__ice_rendered
}); __children.push({
#[cfg(test)]
let __ice_render_source_location = ::ui_lang_runtime::testing::Location::new("screens/overlays.ice", 111, 1, "rendered view node");
#[cfg(test)]
let __ice_render_source = ::ui_lang_runtime::testing::push_render_source(__ice_render_source_location);
let __ice_rendered: __IceElement<'_, __DucktapeMessage> = { let mut __children: ::std::vec::Vec<__IceElement<'_, __DucktapeMessage>> = ::std::vec::Vec::new(); __children.push({
#[cfg(test)]
let __ice_render_source_location = ::ui_lang_runtime::testing::Location::new("screens/overlays.ice", 112, 1, "rendered view node");
#[cfg(test)]
let __ice_render_source = ::ui_lang_runtime::testing::push_render_source(__ice_render_source_location);
let __ice_rendered: __IceElement<'_, __DucktapeMessage> = { let __ice_component_eyebrow_scope_6672 = format!("{}/Eyebrow@6672", __ice_node_scope); ::ui_lang_runtime::rev_memo(1034u64, (__ice_palette.name), ::ui_lang_runtime::grow_stack(|| self.__ice_component_use_183(__ice_palette, __ice_component_eyebrow_scope_6672))).into() };
#[cfg(test)]
drop(__ice_render_source);
__ice_rendered
}); __children.push({
#[cfg(test)]
let __ice_render_source_location = ::ui_lang_runtime::testing::Location::new("screens/overlays.ice", 116, 1, "rendered view node");
#[cfg(test)]
let __ice_render_source = ::ui_lang_runtime::testing::push_render_source(__ice_render_source_location);
let __ice_rendered: __IceElement<'_, __DucktapeMessage> = { let mut __children: ::std::vec::Vec<__IceElement<'_, __DucktapeMessage>> = ::std::vec::Vec::new(); if (!false) { __children.push({
#[cfg(test)]
let __ice_render_source_location = ::ui_lang_runtime::testing::Location::new("screens/overlays.ice", 122, 1, "rendered view node");
#[cfg(test)]
let __ice_render_source = ::ui_lang_runtime::testing::push_render_source(__ice_render_source_location);
let __ice_rendered: __IceElement<'_, __DucktapeMessage> = { let __a11y_key = format!("{}/@container:122", __ice_node_scope); let __container_content: __IceElement<'_, __DucktapeMessage> = {
#[cfg(test)]
let __ice_render_source_location = ::ui_lang_runtime::testing::Location::new("screens/overlays.ice", 133, 1, "rendered view node");
#[cfg(test)]
let __ice_render_source = ::ui_lang_runtime::testing::push_render_source(__ice_render_source_location);
let __ice_rendered: __IceElement<'_, __DucktapeMessage> = { let mut __children: ::std::vec::Vec<__IceElement<'_, __DucktapeMessage>> = ::std::vec::Vec::new(); __children.push({
#[cfg(test)]
let __ice_render_source_location = ::ui_lang_runtime::testing::Location::new("screens/overlays.ice", 134, 1, "rendered view node");
#[cfg(test)]
let __ice_render_source = ::ui_lang_runtime::testing::push_render_source(__ice_render_source_location);
let __ice_rendered: __IceElement<'_, __DucktapeMessage> = { let __a11y_key = format!("{}/@text:134", __ice_node_scope); let __text_value = ("Open").to_string(); let __text = ::iced::widget::text(__text_value.clone()).size(((12.5) as f32).max(f32::EPSILON).min(f32::MAX)).wrapping(::iced::widget::text::Wrapping::None).font(::iced::Font { family: ::iced::font::Family::Name("Geist"), weight: ::iced::font::Weight::Medium, stretch: ::iced::font::Stretch::Normal, style: ::iced::font::Style::Normal }).color(__ice_palette.colors[15]); let __text = ::ui_lang_runtime::selectable_text(__text); ::ui_lang_runtime::accessible(__text, ::ui_lang_runtime::StableId::new(&__a11y_key), ::ui_lang_runtime::Role::Label).logical_id_maybe(::core::cfg!(test).then_some(__a11y_key)).value(__text_value).into() };
#[cfg(test)]
drop(__ice_render_source);
__ice_rendered
}); __children.push({
#[cfg(test)]
let __ice_render_source_location = ::ui_lang_runtime::testing::Location::new("screens/overlays.ice", 140, 1, "rendered view node");
#[cfg(test)]
let __ice_render_source = ::ui_lang_runtime::testing::push_render_source(__ice_render_source_location);
let __ice_rendered: __IceElement<'_, __DucktapeMessage> = { let __a11y_key = format!("{}/@text:140", __ice_node_scope); let __text_value = ("any member posts").to_string(); let __text = ::iced::widget::text(__text_value.clone()).size(((11.0) as f32).max(f32::EPSILON).min(f32::MAX)).wrapping(::iced::widget::text::Wrapping::None).color(__ice_palette.colors[70]); let __text = ::ui_lang_runtime::selectable_text(__text); ::ui_lang_runtime::accessible(__text, ::ui_lang_runtime::StableId::new(&__a11y_key), ::ui_lang_runtime::Role::Label).logical_id_maybe(::core::cfg!(test).then_some(__a11y_key)).value(__text_value).into() };
#[cfg(test)]
drop(__ice_render_source);
__ice_rendered
}); let __child_count = __children.len(); let __children = __children.into_iter().map(|__child| ::ui_lang_runtime::bounded_fill_element(__child, __child_count, false)).collect::<::std::vec::Vec<_>>(); let __layout = ::iced::widget::column(__children).spacing(::ui_lang_runtime::bounded_spacing(2.0, __child_count)).width(::iced::Fill); let __content = ::iced::widget::container(__layout); let __layout_content: __IceElement<'_, __DucktapeMessage> = __content.into(); let __a11y_key = format!("{}/@layout:133", __ice_node_scope); ::ui_lang_runtime::accessible(__layout_content, ::ui_lang_runtime::StableId::new(&__a11y_key), ::ui_lang_runtime::Role::GenericContainer).logical_id_maybe(::core::cfg!(test).then_some(__a11y_key)).into() };
#[cfg(test)]
drop(__ice_render_source);
__ice_rendered
}; let __container = {  ::iced::widget::container(__container_content).padding(::ui_lang_runtime::bounded_padding(8.0, 12.0, 8.0, 12.0)).width(::iced::Fill).style(move |__theme| { let mut __style = ::iced::widget::container::Style::default(); __style.background = ::std::option::Option::Some(::iced::Background::Color(__ice_palette.colors[6])); __style.border.color = __ice_palette.colors[7]; __style.border.width = 1.5 as f32; __style.border.radius = ::iced::border::Radius { top_left: ((9.0) as f32).max(0.0).min(f32::MAX), top_right: ((9.0) as f32).max(0.0).min(f32::MAX), bottom_right: ((9.0) as f32).max(0.0).min(f32::MAX), bottom_left: ((9.0) as f32).max(0.0).min(f32::MAX) }; __style }) }; ::ui_lang_runtime::accessible(__container, ::ui_lang_runtime::StableId::new(&__a11y_key), ::ui_lang_runtime::Role::GenericContainer).logical_id_maybe(::core::cfg!(test).then_some(__a11y_key)).into() };
#[cfg(test)]
drop(__ice_render_source);
__ice_rendered
}); } if (!false) { __children.push({
#[cfg(test)]
let __ice_render_source_location = ::ui_lang_runtime::testing::Location::new("screens/overlays.ice", 199, 1, "rendered view node");
#[cfg(test)]
let __ice_render_source = ::ui_lang_runtime::testing::push_render_source(__ice_render_source_location);
let __ice_rendered: __IceElement<'_, __DucktapeMessage> = { let __a11y_key = format!("{}/@button:199", __ice_node_scope); let __a11y_id = ::ui_lang_runtime::StableId::new(&__a11y_key); let __disabled = false; let __activate = (__ice_cb_7)(); let __button_content: __IceElement<'_, __DucktapeMessage> = {
#[cfg(test)]
let __ice_render_source_location = ::ui_lang_runtime::testing::Location::new("screens/overlays.ice", 205, 1, "rendered view node");
#[cfg(test)]
let __ice_render_source = ::ui_lang_runtime::testing::push_render_source(__ice_render_source_location);
let __ice_rendered: __IceElement<'_, __DucktapeMessage> = { let __a11y_key = format!("{}/@container:205", __ice_node_scope); let __container_content: __IceElement<'_, __DucktapeMessage> = {
#[cfg(test)]
let __ice_render_source_location = ::ui_lang_runtime::testing::Location::new("screens/overlays.ice", 212, 1, "rendered view node");
#[cfg(test)]
let __ice_render_source = ::ui_lang_runtime::testing::push_render_source(__ice_render_source_location);
let __ice_rendered: __IceElement<'_, __DucktapeMessage> = { let mut __children: ::std::vec::Vec<__IceElement<'_, __DucktapeMessage>> = ::std::vec::Vec::new(); __children.push({
#[cfg(test)]
let __ice_render_source_location = ::ui_lang_runtime::testing::Location::new("screens/overlays.ice", 213, 1, "rendered view node");
#[cfg(test)]
let __ice_render_source = ::ui_lang_runtime::testing::push_render_source(__ice_render_source_location);
let __ice_rendered: __IceElement<'_, __DucktapeMessage> = { let __a11y_key = format!("{}/@text:213", __ice_node_scope); let __text_value = ("Members only").to_string(); let __text = ::iced::widget::text(__text_value.clone()).size(((12.5) as f32).max(f32::EPSILON).min(f32::MAX)).wrapping(::iced::widget::text::Wrapping::None).font(::iced::Font { family: ::iced::font::Family::Name("Geist"), weight: ::iced::font::Weight::Medium, stretch: ::iced::font::Stretch::Normal, style: ::iced::font::Style::Normal }).color(__ice_palette.colors[15]); let __text = ::ui_lang_runtime::selectable_text(__text); ::ui_lang_runtime::accessible(__text, ::ui_lang_runtime::StableId::new(&__a11y_key), ::ui_lang_runtime::Role::Label).logical_id_maybe(::core::cfg!(test).then_some(__a11y_key)).value(__text_value).into() };
#[cfg(test)]
drop(__ice_render_source);
__ice_rendered
}); __children.push({
#[cfg(test)]
let __ice_render_source_location = ::ui_lang_runtime::testing::Location::new("screens/overlays.ice", 219, 1, "rendered view node");
#[cfg(test)]
let __ice_render_source = ::ui_lang_runtime::testing::push_render_source(__ice_render_source_location);
let __ice_rendered: __IceElement<'_, __DucktapeMessage> = { let __a11y_key = format!("{}/@text:219", __ice_node_scope); let __text_value = ("added members post").to_string(); let __text = ::iced::widget::text(__text_value.clone()).size(((11.0) as f32).max(f32::EPSILON).min(f32::MAX)).wrapping(::iced::widget::text::Wrapping::None).color(__ice_palette.colors[70]); let __text = ::ui_lang_runtime::selectable_text(__text); ::ui_lang_runtime::accessible(__text, ::ui_lang_runtime::StableId::new(&__a11y_key), ::ui_lang_runtime::Role::Label).logical_id_maybe(::core::cfg!(test).then_some(__a11y_key)).value(__text_value).into() };
#[cfg(test)]
drop(__ice_render_source);
__ice_rendered
}); let __child_count = __children.len(); let __children = __children.into_iter().map(|__child| ::ui_lang_runtime::bounded_fill_element(__child, __child_count, false)).collect::<::std::vec::Vec<_>>(); let __layout = ::iced::widget::column(__children).spacing(::ui_lang_runtime::bounded_spacing(2.0, __child_count)).width(::iced::Fill); let __content = ::iced::widget::container(__layout); let __layout_content: __IceElement<'_, __DucktapeMessage> = __content.into(); let __a11y_key = format!("{}/@layout:212", __ice_node_scope); ::ui_lang_runtime::accessible(__layout_content, ::ui_lang_runtime::StableId::new(&__a11y_key), ::ui_lang_runtime::Role::GenericContainer).logical_id_maybe(::core::cfg!(test).then_some(__a11y_key)).into() };
#[cfg(test)]
drop(__ice_render_source);
__ice_rendered
}; let __container = {  ::iced::widget::container(__container_content).padding(::ui_lang_runtime::bounded_padding(8.0, 12.0, 8.0, 12.0)).width(::iced::Fill) }; ::ui_lang_runtime::accessible(__container, ::ui_lang_runtime::StableId::new(&__a11y_key), ::ui_lang_runtime::Role::GenericContainer).logical_id_maybe(::core::cfg!(test).then_some(__a11y_key)).into() };
#[cfg(test)]
drop(__ice_render_source);
__ice_rendered
}; let __button = ::iced::widget::button(__button_content).padding(::iced::Padding { top: 7.0, right: 12.0, bottom: 7.0, left: 12.0 }).width(::iced::Fill).padding(0.0 as f32).on_press_maybe(if __disabled { None } else { Some(__activate.clone()) }).style(move |__theme, __status| { let mut __style = ::iced::widget::button::primary(__theme, __status); let __background: Option<::iced::Color> = match __status { ::iced::widget::button::Status::Hovered => Some(__ice_palette.colors[14]), ::iced::widget::button::Status::Pressed => Some(__ice_palette.colors[39]), ::iced::widget::button::Status::Disabled => Some(::iced::Color::from_rgba8(0, 0, 0, 0.000000)), _ => Some(::iced::Color::from_rgba8(0, 0, 0, 0.000000)) }; if let Some(__background) = __background { __style.background = Some(::iced::Background::Color(__background)); } __style.text_color = __ice_palette.colors[4]; __style.border.radius = 8.0.into(); __style.background = ::std::option::Option::Some(::iced::Background::Color(__ice_palette.colors[3])); __style.text_color = __ice_palette.colors[4]; __style.border.color = __ice_palette.colors[39]; __style.border.width = 1.5 as f32; __style.border.radius = ::iced::border::Radius { top_left: ((9.0) as f32).max(0.0).min(f32::MAX), top_right: ((9.0) as f32).max(0.0).min(f32::MAX), bottom_right: ((9.0) as f32).max(0.0).min(f32::MAX), bottom_left: ((9.0) as f32).max(0.0).min(f32::MAX) }; match __status { ::iced::widget::button::Status::Hovered => { __style.background = ::std::option::Option::Some(::iced::Background::Color(__ice_palette.colors[6])); __style.text_color = __ice_palette.colors[4]; __style.border.color = __ice_palette.colors[40]; } ::iced::widget::button::Status::Pressed => { __style.background = ::std::option::Option::Some(::iced::Background::Color(__ice_palette.colors[55])); __style.text_color = __ice_palette.colors[4]; } _ => {} } if matches!(__status, ::iced::widget::button::Status::Disabled) { if let Some(::iced::Background::Color(mut __color)) = __style.background { __color.a *= 0.5; __style.background = Some(::iced::Background::Color(__color)); } __style.text_color.a *= 0.5; } __style }); ::ui_lang_runtime::accessible(__button, __a11y_id, ::ui_lang_runtime::Role::Button).logical_id_maybe(::core::cfg!(test).then_some(&*__a11y_key)).focus_id(::iced::widget::Id::from(__a11y_key)).label("Members-only posting".to_owned()).disabled(__disabled).on_activate_maybe(if __disabled { None } else { Some(__activate) }).focus_ring(__ice_palette.colors[42], 8.0).into() };
#[cfg(test)]
drop(__ice_render_source);
__ice_rendered
}); } let __child_count = __children.len(); let __children = __children.into_iter().map(|__child| ::ui_lang_runtime::bounded_fill_element(__child, __child_count, true)).collect::<::std::vec::Vec<_>>(); let __layout = ::iced::widget::row(__children).spacing(::ui_lang_runtime::bounded_spacing(8.0, __child_count)).width(::iced::Fill).align_y(::iced::alignment::Vertical::Center); let __content = ::iced::widget::container(__layout); let __layout_content: __IceElement<'_, __DucktapeMessage> = __content.into(); let __a11y_key = format!("{}/@layout:116", __ice_node_scope); ::ui_lang_runtime::accessible(__layout_content, ::ui_lang_runtime::StableId::new(&__a11y_key), ::ui_lang_runtime::Role::GenericContainer).logical_id_maybe(::core::cfg!(test).then_some(__a11y_key)).into() };
#[cfg(test)]
drop(__ice_render_source);
__ice_rendered
}); let __child_count = __children.len(); let __children = __children.into_iter().map(|__child| ::ui_lang_runtime::bounded_fill_element(__child, __child_count, false)).collect::<::std::vec::Vec<_>>(); let __layout = ::iced::widget::column(__children).spacing(::ui_lang_runtime::bounded_spacing(6.0, __child_count)).width(::iced::Fill); let __content = ::iced::widget::container(__layout); let __layout_content: __IceElement<'_, __DucktapeMessage> = __content.into(); let __a11y_key = format!("{}/@layout:111", __ice_node_scope); ::ui_lang_runtime::accessible(__layout_content, ::ui_lang_runtime::StableId::new(&__a11y_key), ::ui_lang_runtime::Role::GenericContainer).logical_id_maybe(::core::cfg!(test).then_some(__a11y_key)).into() };
#[cfg(test)]
drop(__ice_render_source);
__ice_rendered
}); __children.push({
#[cfg(test)]
let __ice_render_source_location = ::ui_lang_runtime::testing::Location::new("screens/overlays.ice", 227, 1, "rendered view node");
#[cfg(test)]
let __ice_render_source = ::ui_lang_runtime::testing::push_render_source(__ice_render_source_location);
let __ice_rendered: __IceElement<'_, __DucktapeMessage> = { let mut __children: ::std::vec::Vec<__IceElement<'_, __DucktapeMessage>> = ::std::vec::Vec::new(); __children.push({
#[cfg(test)]
let __ice_render_source_location = ::ui_lang_runtime::testing::Location::new("screens/overlays.ice", 232, 1, "rendered view node");
#[cfg(test)]
let __ice_render_source = ::ui_lang_runtime::testing::push_render_source(__ice_render_source_location);
let __ice_rendered: __IceElement<'_, __DucktapeMessage> = { let __a11y_key = format!("{}/@button:232", __ice_node_scope); let __a11y_id = ::ui_lang_runtime::StableId::new(&__a11y_key); let __disabled = false; let __activate = (__ice_cb_6)(); let __button_content: __IceElement<'_, __DucktapeMessage> = { let __button_inner: __IceElement<'_, __DucktapeMessage> = ::iced::widget::text("Cancel").size(12.5).font(::iced::Font { weight: ::iced::font::Weight::Semibold, ..Self::default_font() }).into(); ::iced::widget::container(__button_inner).height(::iced::Fill).align_y(::iced::alignment::Vertical::Center).into() }; let __button = ::iced::widget::button(__button_content).padding(::iced::Padding { top: 11.0, right: 16.0, bottom: 11.0, left: 16.0 }).width(::iced::Fill).height(38.0 as f32).padding(9.0 as f32).on_press_maybe(if __disabled { None } else { Some(__activate.clone()) }).style(move |__theme, __status| { let mut __style = ::iced::widget::button::primary(__theme, __status); let __background: Option<::iced::Color> = match __status { ::iced::widget::button::Status::Hovered => Some(__ice_palette.colors[14]), ::iced::widget::button::Status::Pressed => Some(__ice_palette.colors[6]), ::iced::widget::button::Status::Disabled => Some(__ice_palette.colors[12]), _ => Some(__ice_palette.colors[12]) }; if let Some(__background) = __background { __style.background = Some(::iced::Background::Color(__background)); } __style.text_color = __ice_palette.colors[13]; __style.border.width = 1.0; __style.border.color = __ice_palette.colors[40]; __style.border.radius = 9.0.into(); if matches!(__status, ::iced::widget::button::Status::Disabled) { if let Some(::iced::Background::Color(mut __color)) = __style.background { __color.a *= 0.5; __style.background = Some(::iced::Background::Color(__color)); } __style.text_color.a *= 0.5; } __style }); ::ui_lang_runtime::accessible(__button, __a11y_id, ::ui_lang_runtime::Role::Button).logical_id_maybe(::core::cfg!(test).then_some(&*__a11y_key)).focus_id(::iced::widget::Id::from(__a11y_key)).label("Cancel").disabled(__disabled).on_activate_maybe(if __disabled { None } else { Some(__activate) }).focus_ring(__ice_palette.colors[42], 9.0).into() };
#[cfg(test)]
drop(__ice_render_source);
__ice_rendered
}); __children.push({
#[cfg(test)]
let __ice_render_source_location = ::ui_lang_runtime::testing::Location::new("screens/overlays.ice", 239, 1, "rendered view node");
#[cfg(test)]
let __ice_render_source = ::ui_lang_runtime::testing::push_render_source(__ice_render_source_location);
let __ice_rendered: __IceElement<'_, __DucktapeMessage> = { let __a11y_key = format!("{}/@button:239", __ice_node_scope); let __a11y_id = ::ui_lang_runtime::StableId::new(&__a11y_key); let __disabled = (((false || false) || (!true)) || ((self.channel_draft).trim().to_owned()).is_empty()); let __activate = (__ice_cb_1)(); let __button_content: __IceElement<'_, __DucktapeMessage> = { let __button_inner: __IceElement<'_, __DucktapeMessage> = ::iced::widget::text("Create →").size(12.5).font(::iced::Font { weight: ::iced::font::Weight::Semibold, ..Self::default_font() }).into(); ::iced::widget::container(__button_inner).height(::iced::Fill).align_y(::iced::alignment::Vertical::Center).into() }; let __button = ::iced::widget::button(__button_content).padding(::iced::Padding { top: 11.0, right: 16.0, bottom: 11.0, left: 16.0 }).width(::iced::Fill).height(38.0 as f32).padding(9.0 as f32).on_press_maybe(if __disabled { None } else { Some(__activate.clone()) }).style(move |__theme, __status| { let mut __style = ::iced::widget::button::primary(__theme, __status); let __background: Option<::iced::Color> = match __status { ::iced::widget::button::Status::Hovered => Some(__ice_palette.colors[8]), ::iced::widget::button::Status::Pressed => Some({ let mut __color = __ice_palette.colors[7]; __color.a = 0.800000; __color }), ::iced::widget::button::Status::Disabled => Some(__ice_palette.colors[7]), _ => Some(__ice_palette.colors[7]) }; if let Some(__background) = __background { __style.background = Some(::iced::Background::Color(__background)); } __style.text_color = __ice_palette.colors[9]; __style.border.radius = 9.0.into(); if matches!(__status, ::iced::widget::button::Status::Disabled) { __style.background = Some(__ice_palette.colors[10].into()); __style.text_color = __ice_palette.colors[11]; } __style }); ::ui_lang_runtime::accessible(__button, __a11y_id, ::ui_lang_runtime::Role::Button).logical_id_maybe(::core::cfg!(test).then_some(&*__a11y_key)).focus_id(::iced::widget::Id::from(__a11y_key)).label("Create →").disabled(__disabled).on_activate_maybe(if __disabled { None } else { Some(__activate) }).focus_ring(__ice_palette.colors[42], 9.0).into() };
#[cfg(test)]
drop(__ice_render_source);
__ice_rendered
}); let __child_count = __children.len(); let __children = __children.into_iter().map(|__child| ::ui_lang_runtime::bounded_fill_element(__child, __child_count, true)).collect::<::std::vec::Vec<_>>(); let __layout = ::iced::widget::row(__children).spacing(::ui_lang_runtime::bounded_spacing(8.0, __child_count)).width(::iced::Fill).align_y(::iced::alignment::Vertical::Center); let __content = ::iced::widget::container(__layout); let __layout_content: __IceElement<'_, __DucktapeMessage> = __content.into(); let __a11y_key = format!("{}/@layout:227", __ice_node_scope); ::ui_lang_runtime::accessible(__layout_content, ::ui_lang_runtime::StableId::new(&__a11y_key), ::ui_lang_runtime::Role::GenericContainer).logical_id_maybe(::core::cfg!(test).then_some(__a11y_key)).into() };
#[cfg(test)]
drop(__ice_render_source);
__ice_rendered
}); let __child_count = __children.len(); let __children = __children.into_iter().map(|__child| ::ui_lang_runtime::bounded_fill_element(__child, __child_count, false)).collect::<::std::vec::Vec<_>>(); let __layout = ::iced::widget::column(__children).spacing(::ui_lang_runtime::bounded_spacing(13.0, __child_count)).width(::iced::Fill); let __content = ::iced::widget::container(__layout); let __layout_content: __IceElement<'_, __DucktapeMessage> = __content.into(); let __a11y_key = format!("{}/@layout:65", __ice_node_scope); ::ui_lang_runtime::accessible(__layout_content, ::ui_lang_runtime::StableId::new(&__a11y_key), ::ui_lang_runtime::Role::GenericContainer).logical_id_maybe(::core::cfg!(test).then_some(__a11y_key)).into() };
#[cfg(test)]
drop(__ice_render_source);
__ice_rendered
}; __slot_content })();
#[cfg(test)]
drop(__ice_render_source);
__ice_rendered
}); let __child_count = __children.len(); let __children = __children.into_iter().map(|__child| ::ui_lang_runtime::bounded_fill_element(__child, __child_count, false)).collect::<::std::vec::Vec<_>>(); let __layout = ::iced::widget::column(__children).spacing(::ui_lang_runtime::bounded_spacing(13.0, __child_count)).padding(::ui_lang_runtime::bounded_padding(20.0, 22.0, 22.0, 22.0)).width(::iced::Fill); let __content = ::iced::widget::container(__layout); let __layout_content: __IceElement<'_, __DucktapeMessage> = __content.into(); let __a11y_key = format!("{}/@layout:97", __ice_use_scope); ::ui_lang_runtime::accessible(__layout_content, ::ui_lang_runtime::StableId::new(&__a11y_key), ::ui_lang_runtime::Role::GenericContainer).logical_id_maybe(::core::cfg!(test).then_some(__a11y_key)).into() };
#[cfg(test)]
drop(__ice_render_source);
__ice_rendered
}; let __container = {  ::iced::widget::container(__container_content).id(::iced::widget::Id::from(__ice_node_scope.clone())).width(418.0 as f32).style(move |__theme| { let mut __style = ::iced::widget::container::Style::default(); __style.background = ::std::option::Option::Some(::iced::Background::Color(__ice_palette.colors[3])); __style.border.color = __ice_palette.colors[39]; __style.border.width = 1.0 as f32; __style.border.radius = ::iced::border::Radius { top_left: ((14.0) as f32).max(0.0).min(f32::MAX), top_right: ((14.0) as f32).max(0.0).min(f32::MAX), bottom_right: ((14.0) as f32).max(0.0).min(f32::MAX), bottom_left: ((14.0) as f32).max(0.0).min(f32::MAX) }; __style.shadow.color = __ice_palette.colors[48]; __style.shadow.offset.y = 24.0 as f32; __style.shadow.blur_radius = 60.0 as f32; __style }) }; ::ui_lang_runtime::accessible(__container, ::ui_lang_runtime::StableId::new(&__a11y_key), ::ui_lang_runtime::Role::GenericContainer).logical_id_maybe(::core::cfg!(test).then_some(__a11y_key)).into() } };
#[cfg(test)]
drop(__ice_render_source);
__ice_rendered
}; __component_content }
#[cfg(test)]
pub(super) fn __ice_component_use_185(&self, __ice_palette: __IcePalette, __ice_use_scope: ::std::string::String) -> __IceElement<'_, __DucktapeMessage> { let __component_content: __IceElement<'_, __DucktapeMessage> = {
#[cfg(test)]
let __ice_render_source_location = ::ui_lang_runtime::testing::Location::new("components/overlay.ice", 124, 1, "rendered view node");
#[cfg(test)]
let __ice_render_source = ::ui_lang_runtime::testing::push_render_source(__ice_render_source_location);
let __ice_rendered: __IceElement<'_, __DucktapeMessage> = { let __ice_node_scope = format!("{}/root", __ice_use_scope); { let __a11y_key = __ice_node_scope.as_str(); let __container_content: __IceElement<'_, __DucktapeMessage> = {
#[cfg(test)]
let __ice_render_source_location = ::ui_lang_runtime::testing::Location::new("components/overlay.ice", 133, 1, "rendered view node");
#[cfg(test)]
let __ice_render_source = ::ui_lang_runtime::testing::push_render_source(__ice_render_source_location);
let __ice_rendered: __IceElement<'_, __DucktapeMessage> = { let mut __children: ::std::vec::Vec<__IceElement<'_, __DucktapeMessage>> = ::std::vec::Vec::new(); if ("info" == "error") { __children.push({
#[cfg(test)]
let __ice_render_source_location = ::ui_lang_runtime::testing::Location::new("components/overlay.ice", 136, 1, "rendered view node");
#[cfg(test)]
let __ice_render_source = ::ui_lang_runtime::testing::push_render_source(__ice_render_source_location);
let __ice_rendered: __IceElement<'_, __DucktapeMessage> = { let __a11y_key = format!("{}/@container:136", __ice_use_scope); let __container_content: __IceElement<'_, __DucktapeMessage> = {
#[cfg(test)]
let __ice_render_source_location = ::ui_lang_runtime::testing::Location::new("components/overlay.ice", 142, 1, "rendered view node");
#[cfg(test)]
let __ice_render_source = ::ui_lang_runtime::testing::push_render_source(__ice_render_source_location);
let __ice_rendered: __IceElement<'_, __DucktapeMessage> = ::iced::widget::space().width(1.0 as f32).height(1.0 as f32).into();
#[cfg(test)]
drop(__ice_render_source);
__ice_rendered
}; let __container = {  ::iced::widget::container(__container_content).width(6.0 as f32).height(6.0 as f32).style(move |__theme| { let mut __style = ::iced::widget::container::Style::default(); __style.background = ::std::option::Option::Some(::iced::Background::Color(__ice_palette.colors[24])); __style.border.radius = ::iced::border::Radius { top_left: ((3.0) as f32).max(0.0).min(f32::MAX), top_right: ((3.0) as f32).max(0.0).min(f32::MAX), bottom_right: ((3.0) as f32).max(0.0).min(f32::MAX), bottom_left: ((3.0) as f32).max(0.0).min(f32::MAX) }; __style }) }; ::ui_lang_runtime::accessible(__container, ::ui_lang_runtime::StableId::new(&__a11y_key), ::ui_lang_runtime::Role::GenericContainer).logical_id_maybe(::core::cfg!(test).then_some(__a11y_key)).into() };
#[cfg(test)]
drop(__ice_render_source);
__ice_rendered
}); } if ((!("info" == "error")) && ("info" == "warning")) { __children.push({
#[cfg(test)]
let __ice_render_source_location = ::ui_lang_runtime::testing::Location::new("components/overlay.ice", 144, 1, "rendered view node");
#[cfg(test)]
let __ice_render_source = ::ui_lang_runtime::testing::push_render_source(__ice_render_source_location);
let __ice_rendered: __IceElement<'_, __DucktapeMessage> = { let __a11y_key = format!("{}/@container:144", __ice_use_scope); let __container_content: __IceElement<'_, __DucktapeMessage> = {
#[cfg(test)]
let __ice_render_source_location = ::ui_lang_runtime::testing::Location::new("components/overlay.ice", 150, 1, "rendered view node");
#[cfg(test)]
let __ice_render_source = ::ui_lang_runtime::testing::push_render_source(__ice_render_source_location);
let __ice_rendered: __IceElement<'_, __DucktapeMessage> = ::iced::widget::space().width(1.0 as f32).height(1.0 as f32).into();
#[cfg(test)]
drop(__ice_render_source);
__ice_rendered
}; let __container = {  ::iced::widget::container(__container_content).width(6.0 as f32).height(6.0 as f32).style(move |__theme| { let mut __style = ::iced::widget::container::Style::default(); __style.background = ::std::option::Option::Some(::iced::Background::Color(__ice_palette.colors[34])); __style.border.radius = ::iced::border::Radius { top_left: ((3.0) as f32).max(0.0).min(f32::MAX), top_right: ((3.0) as f32).max(0.0).min(f32::MAX), bottom_right: ((3.0) as f32).max(0.0).min(f32::MAX), bottom_left: ((3.0) as f32).max(0.0).min(f32::MAX) }; __style }) }; ::ui_lang_runtime::accessible(__container, ::ui_lang_runtime::StableId::new(&__a11y_key), ::ui_lang_runtime::Role::GenericContainer).logical_id_maybe(::core::cfg!(test).then_some(__a11y_key)).into() };
#[cfg(test)]
drop(__ice_render_source);
__ice_rendered
}); } if (!(("info" == "error") || ("info" == "warning"))) { __children.push({
#[cfg(test)]
let __ice_render_source_location = ::ui_lang_runtime::testing::Location::new("components/overlay.ice", 152, 1, "rendered view node");
#[cfg(test)]
let __ice_render_source = ::ui_lang_runtime::testing::push_render_source(__ice_render_source_location);
let __ice_rendered: __IceElement<'_, __DucktapeMessage> = { let __a11y_key = format!("{}/@container:152", __ice_use_scope); let __container_content: __IceElement<'_, __DucktapeMessage> = {
#[cfg(test)]
let __ice_render_source_location = ::ui_lang_runtime::testing::Location::new("components/overlay.ice", 158, 1, "rendered view node");
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
}); } __children.push({
#[cfg(test)]
let __ice_render_source_location = ::ui_lang_runtime::testing::Location::new("components/overlay.ice", 159, 1, "rendered view node");
#[cfg(test)]
let __ice_render_source = ::ui_lang_runtime::testing::push_render_source(__ice_render_source_location);
let __ice_rendered: __IceElement<'_, __DucktapeMessage> = { let __a11y_key = format!("{}/@text:159", __ice_use_scope); let __text_value = ("").to_string(); let __text = ::iced::widget::text(__text_value.clone()).size(((12.5) as f32).max(f32::EPSILON).min(f32::MAX)).wrapping(::iced::widget::text::Wrapping::None).color(__ice_palette.colors[38]); let __text = ::ui_lang_runtime::selectable_text(__text); ::ui_lang_runtime::accessible(__text, ::ui_lang_runtime::StableId::new(&__a11y_key), ::ui_lang_runtime::Role::Label).logical_id_maybe(::core::cfg!(test).then_some(__a11y_key)).value(__text_value).into() };
#[cfg(test)]
drop(__ice_render_source);
__ice_rendered
}); let __child_count = __children.len(); let __children = __children.into_iter().map(|__child| ::ui_lang_runtime::bounded_fill_element(__child, __child_count, true)).collect::<::std::vec::Vec<_>>(); let __layout = ::iced::widget::row(__children).spacing(::ui_lang_runtime::bounded_spacing(9.0, __child_count)).align_y(::iced::alignment::Vertical::Center); let __content = ::iced::widget::container(__layout); let __layout_content: __IceElement<'_, __DucktapeMessage> = __content.into(); let __a11y_key = format!("{}/@layout:133", __ice_use_scope); ::ui_lang_runtime::accessible(__layout_content, ::ui_lang_runtime::StableId::new(&__a11y_key), ::ui_lang_runtime::Role::GenericContainer).logical_id_maybe(::core::cfg!(test).then_some(__a11y_key)).into() };
#[cfg(test)]
drop(__ice_render_source);
__ice_rendered
}; let __container = {  ::iced::widget::container(__container_content).id(::iced::widget::Id::from(__ice_node_scope.clone())).padding(::ui_lang_runtime::bounded_padding(10.0, 16.0, 10.0, 16.0)).style(move |__theme| { let mut __style = ::iced::widget::container::Style::default(); __style.background = ::std::option::Option::Some(::iced::Background::Color(__ice_palette.colors[37])); __style.border.radius = ::iced::border::Radius { top_left: ((10.0) as f32).max(0.0).min(f32::MAX), top_right: ((10.0) as f32).max(0.0).min(f32::MAX), bottom_right: ((10.0) as f32).max(0.0).min(f32::MAX), bottom_left: ((10.0) as f32).max(0.0).min(f32::MAX) }; __style.shadow.color = __ice_palette.colors[47]; __style.shadow.offset.y = 6.0 as f32; __style.shadow.blur_radius = 18.0 as f32; __style }) }; ::ui_lang_runtime::accessible(__container, ::ui_lang_runtime::StableId::new(&__a11y_key), ::ui_lang_runtime::Role::GenericContainer).logical_id_maybe(::core::cfg!(test).then_some(__a11y_key)).into() } };
#[cfg(test)]
drop(__ice_render_source);
__ice_rendered
}; __component_content }
}
}
