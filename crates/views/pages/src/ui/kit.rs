#[allow(warnings, clippy::all)]
mod __ice_group_kit_7309d6fe {
use super::*;
impl super::PagesView {
pub(super) fn __ice_component_use_1(&self, __ice_palette: __IcePalette, __ice_use_scope: ::std::string::String, __ice_cb_0: impl Fn() -> __PagesViewMessage + Clone + 'static, __ice_cb_1: impl Fn(::std::string::String) -> __PagesViewMessage + Clone + 'static, __ice_cb_2: impl Fn() -> __PagesViewMessage + Clone + 'static, __ice_cb_3: impl Fn() -> __PagesViewMessage + Clone + 'static, __ice_cb_4: impl Fn() -> __PagesViewMessage + Clone + 'static, __ice_cb_5: impl Fn(::std::string::String, ::std::string::String) -> __PagesViewMessage + Clone + 'static, __ice_cb_6: impl Fn() -> __PagesViewMessage + Clone + 'static, __ice_cb_7: impl Fn() -> __PagesViewMessage + Clone + 'static, __ice_cb_8: impl Fn() -> __PagesViewMessage + Clone + 'static, __ice_cb_9: impl Fn(::std::string::String) -> __PagesViewMessage + Clone + 'static, __ice_cb_10: impl Fn(f64, f64) -> __PagesViewMessage + Clone + 'static, __ice_cb_11: impl Fn(::std::string::String) -> __PagesViewMessage + Clone + 'static, __ice_cb_12: impl Fn(::std::string::String, ::std::string::String) -> __PagesViewMessage + Clone + 'static, __ice_cb_13: impl Fn() -> __PagesViewMessage + Clone + 'static, __ice_cb_14: impl Fn(::std::string::String) -> __PagesViewMessage + Clone + 'static, __ice_cb_15: impl Fn(f64, f64) -> __PagesViewMessage + Clone + 'static, __ice_cb_16: impl Fn(f64, f64) -> __PagesViewMessage + Clone + 'static, __ice_cb_17: impl Fn(::std::string::String, bool) -> __PagesViewMessage + Clone + 'static, __ice_cb_18: impl Fn() -> __PagesViewMessage + Clone + 'static, __ice_cb_19: impl Fn(::std::string::String) -> __PagesViewMessage + Clone + 'static, __ice_cb_20: impl Fn() -> __PagesViewMessage + Clone + 'static, __ice_cb_21: impl Fn() -> __PagesViewMessage + Clone + 'static, __ice_cb_22: impl Fn() -> __PagesViewMessage + Clone + 'static, __ice_cb_23: impl Fn() -> __PagesViewMessage + Clone + 'static, __ice_cb_24: impl Fn(::std::string::String) -> __PagesViewMessage + Clone + 'static, __ice_cb_25: impl Fn(::std::string::String) -> __PagesViewMessage + Clone + 'static, __ice_cb_26: impl Fn() -> __PagesViewMessage + Clone + 'static) -> __IceElement<'_, __PagesViewMessage> { let __component_content: __IceElement<'_, __PagesViewMessage> = {
#[cfg(test)]
let __ice_render_source_location = ::ui_lang_runtime::testing::Location::new("src/ui/kit.ice", 6, 1, "rendered view node");
#[cfg(test)]
let __ice_render_source = ::ui_lang_runtime::testing::push_render_source(__ice_render_source_location);
let __ice_rendered: __IceElement<'_, __PagesViewMessage> = { let __ice_node_scope = format!("{}/root", __ice_use_scope); { let mut __children: ::std::vec::Vec<__IceElement<'_, __PagesViewMessage>> = ::std::vec::Vec::new(); __children.push({
#[cfg(test)]
let __ice_render_source_location = ::ui_lang_runtime::testing::Location::new("src/ui/kit.ice", 7, 1, "rendered view node");
#[cfg(test)]
let __ice_render_source = ::ui_lang_runtime::testing::push_render_source(__ice_render_source_location);
let __ice_rendered: __IceElement<'_, __PagesViewMessage> = ::ui_lang_guest::wire::Node::Container { shadow: ::ui_lang_guest::wire::Shadow { color: ::std::option::Option::None, x: ::std::option::Option::None, y: ::std::option::Option::None, blur: ::std::option::Option::None }, max_width: ::std::option::Option::None, max_height: ::std::option::Option::None, clip: false, key: format!("{}/@container:7", __ice_use_scope), width: ::std::option::Option::Some(::ui_lang_guest::wire::Length::Fill), height: ::std::option::Option::Some(::ui_lang_guest::wire::Length::Fixed((50.0) as f32)), padding: ::std::option::Option::Some(::ui_lang_guest::wire::Edges { top: (0.0) as f32, right: (14.0) as f32, bottom: (0.0) as f32, left: (14.0) as f32 }), align_x: ::std::option::Option::None, align_y: ::std::option::Option::None, background: (::std::option::Option::None).map(::ui_lang_guest::wire::Background::Color), border: ::std::option::Option::None, snap: ::std::option::Option::None, content: ::std::boxed::Box::new({
#[cfg(test)]
let __ice_render_source_location = ::ui_lang_runtime::testing::Location::new("src/ui/kit.ice", 13, 1, "rendered view node");
#[cfg(test)]
let __ice_render_source = ::ui_lang_runtime::testing::push_render_source(__ice_render_source_location);
let __ice_rendered: __IceElement<'_, __PagesViewMessage> = { let mut __children: ::std::vec::Vec<__IceElement<'_, __PagesViewMessage>> = ::std::vec::Vec::new(); __children.push({
#[cfg(test)]
let __ice_render_source_location = ::ui_lang_runtime::testing::Location::new("src/ui/kit.ice", 19, 1, "rendered view node");
#[cfg(test)]
let __ice_render_source = ::ui_lang_runtime::testing::push_render_source(__ice_render_source_location);
let __ice_rendered: __IceElement<'_, __PagesViewMessage> = ::ui_lang_guest::wire::Node::Text { options: ::ui_lang_guest::wire::TextOptions { height: ::std::option::Option::None, align_y: ::std::option::Option::None, line_height: ::std::option::Option::None, shaping: ::std::option::Option::None, wrapping: ::std::option::Option::Some(::ui_lang_guest::wire::Wrapping::None), tracking: 0.0f32, font: ::std::option::Option::Some(::ui_lang_guest::wire::NamedFont { family: ::ui_lang_guest::wire::FontFamily::Named("Geist".into()), weight: ::ui_lang_guest::wire::Weight::Semibold, stretch: ::ui_lang_guest::wire::FontStretch::Normal, style: ::ui_lang_guest::wire::FontStyle::Normal }) }, key: format!("{}/@text:19", __ice_use_scope), size: ::std::option::Option::Some(((13.5) as f32).max(f32::EPSILON).min(f32::MAX)), color: ::std::option::Option::Some({ let __color: ::iced::Color = __ice_palette.colors[4]; ::ui_lang_guest::wire::Rgba([__color.r, __color.g, __color.b, __color.a]) }), font: ::ui_lang_guest::wire::Font { monospace: false, weight: ::ui_lang_guest::wire::Weight::Normal }, width: ::std::option::Option::None, align_x: ::std::option::Option::None, content: ("Pages".to_owned()).to_string() };
#[cfg(test)]
drop(__ice_render_source);
__ice_rendered
}); __children.push({
#[cfg(test)]
let __ice_render_source_location = ::ui_lang_runtime::testing::Location::new("src/ui/kit.ice", 25, 1, "rendered view node");
#[cfg(test)]
let __ice_render_source = ::ui_lang_runtime::testing::push_render_source(__ice_render_source_location);
let __ice_rendered: __IceElement<'_, __PagesViewMessage> = ::ui_lang_guest::wire::Node::Text { options: ::ui_lang_guest::wire::TextOptions { height: ::std::option::Option::None, align_y: ::std::option::Option::None, line_height: ::std::option::Option::None, shaping: ::std::option::Option::None, wrapping: ::std::option::Option::Some(::ui_lang_guest::wire::Wrapping::None), tracking: 0.0f32, font: ::std::option::Option::Some(::ui_lang_guest::wire::NamedFont { family: ::ui_lang_guest::wire::FontFamily::Named("Geist Mono".into()), weight: ::ui_lang_guest::wire::Weight::Medium, stretch: ::ui_lang_guest::wire::FontStretch::Normal, style: ::ui_lang_guest::wire::FontStyle::Normal }) }, key: format!("{}/@text:25", __ice_use_scope), size: ::std::option::Option::Some(((10.5) as f32).max(f32::EPSILON).min(f32::MAX)), color: ::std::option::Option::Some({ let __color: ::iced::Color = __ice_palette.colors[72]; ::ui_lang_guest::wire::Rgba([__color.r, __color.g, __color.b, __color.a]) }), font: ::ui_lang_guest::wire::Font { monospace: false, weight: ::ui_lang_guest::wire::Weight::Normal }, width: ::std::option::Option::None, align_x: ::std::option::Option::None, content: (((self.pages).len() as i64)).to_string() };
#[cfg(test)]
drop(__ice_render_source);
__ice_rendered
}); __children.push({
#[cfg(test)]
let __ice_render_source_location = ::ui_lang_runtime::testing::Location::new("src/ui/kit.ice", 31, 1, "rendered view node");
#[cfg(test)]
let __ice_render_source = ::ui_lang_runtime::testing::push_render_source(__ice_render_source_location);
let __ice_rendered: __IceElement<'_, __PagesViewMessage> = ::ui_lang_guest::wire::Node::Space { width: ::std::option::Option::Some(::ui_lang_guest::wire::Length::Fill), height: ::std::option::Option::None };
#[cfg(test)]
drop(__ice_render_source);
__ice_rendered
}); __children.push({
#[cfg(test)]
let __ice_render_source_location = ::ui_lang_runtime::testing::Location::new("src/ui/kit.ice", 32, 1, "rendered view node");
#[cfg(test)]
let __ice_render_source = ::ui_lang_runtime::testing::push_render_source(__ice_render_source_location);
let __ice_rendered: __IceElement<'_, __PagesViewMessage> = (|| { let __slot_content: __IceElement<'_, __PagesViewMessage> = {
#[cfg(test)]
let __ice_render_source_location = ::ui_lang_runtime::testing::Location::new("src/ui/pages.ice", 50, 1, "rendered view node");
#[cfg(test)]
let __ice_render_source = ::ui_lang_runtime::testing::push_render_source(__ice_render_source_location);
let __ice_rendered: __IceElement<'_, __PagesViewMessage> = { let mut __children: ::std::vec::Vec<__IceElement<'_, __PagesViewMessage>> = ::std::vec::Vec::new(); if (!self.page_create_open) { __children.push({
#[cfg(test)]
let __ice_render_source_location = ::ui_lang_runtime::testing::Location::new("src/ui/pages.ice", 52, 1, "rendered view node");
#[cfg(test)]
let __ice_render_source = ::ui_lang_runtime::testing::push_render_source(__ice_render_source_location);
let __ice_rendered: __IceElement<'_, __PagesViewMessage> = ::ui_lang_guest::wire::Node::Button { checked: ::std::option::Option::None, expanded: ::std::option::Option::Some(self.page_create_open), description: ::std::option::Option::None, key: format!("{}/@button:52", __ice_use_scope), content: ::ui_lang_guest::wire::ButtonContent::Child(::std::boxed::Box::new({
#[cfg(test)]
let __ice_render_source_location = ::ui_lang_runtime::testing::Location::new("src/ui/pages.ice", 59, 1, "rendered view node");
#[cfg(test)]
let __ice_render_source = ::ui_lang_runtime::testing::push_render_source(__ice_render_source_location);
let __ice_rendered: __IceElement<'_, __PagesViewMessage> = ::ui_lang_runtime::grow_stack(|| self.__ice_component_use_0(__ice_palette, format!("{}/Icon@680", __ice_use_scope)));
#[cfg(test)]
drop(__ice_render_source);
__ice_rendered
})), label: ::std::option::Option::Some(::std::string::String::from("New page".to_owned())), on_press: if (((self.loading || (self.busy || (!(self.host_error).is_empty()))) || (!self.connected))) { ::std::option::Option::None } else { ::std::option::Option::Some(::ui_lang_guest::slots::message((__ice_cb_21)())) }, width: ::std::option::Option::None, height: ::std::option::Option::None, padding: ::std::option::Option::Some(::ui_lang_guest::wire::Edges::all((0.0) as f32)), style: ::ui_lang_guest::wire::ButtonStyle { preset: ::ui_lang_guest::wire::ButtonPreset::Primary, recipe: Some(::ui_lang_guest::wire::ButtonRecipe { base: ::ui_lang_guest::wire::Face { background: ::std::option::Option::Some({ let __color: ::iced::Color = ::iced::Color::from_rgba8(0, 0, 0, 0.000000); ::ui_lang_guest::wire::Rgba([__color.r, __color.g, __color.b, __color.a]) }), text: ::std::option::Option::Some({ let __color: ::iced::Color = __ice_palette.colors[4]; ::ui_lang_guest::wire::Rgba([__color.r, __color.g, __color.b, __color.a]) }), border: ::std::option::Option::Some(::ui_lang_guest::wire::Border { color: ::std::option::Option::None, width: ::std::option::Option::None, radius: ::std::option::Option::Some([7.0; 4]) }) }, hover_background: ::std::option::Option::Some({ let __color: ::iced::Color = __ice_palette.colors[14]; ::ui_lang_guest::wire::Rgba([__color.r, __color.g, __color.b, __color.a]) }), pressed_background: ::std::option::Option::Some({ let __color: ::iced::Color = __ice_palette.colors[39]; ::ui_lang_guest::wire::Rgba([__color.r, __color.g, __color.b, __color.a]) }), disabled_background: ::std::option::Option::None, disabled_text: ::std::option::Option::None, disabled_opacity: ::std::option::Option::Some(0.5f32), focus_ring: ::std::option::Option::Some({ let __color: ::iced::Color = __ice_palette.colors[42]; ::ui_lang_guest::wire::Rgba([__color.r, __color.g, __color.b, __color.a]) }), text_size: ::std::option::Option::Some(13.5f32), line_height: ::std::option::Option::None, font: Some(::ui_lang_guest::wire::NamedFont { family: ::ui_lang_guest::wire::FontFamily::Named("Geist".into()), weight: ::ui_lang_guest::wire::Weight::Normal, stretch: ::ui_lang_guest::wire::FontStretch::Normal, style: ::ui_lang_guest::wire::FontStyle::Normal }) }), active: ::ui_lang_guest::wire::Face { background: ::std::option::Option::Some({ let __color: ::iced::Color = ::iced::Color::from_rgba8(0, 0, 0, 0.000000); ::ui_lang_guest::wire::Rgba([__color.r, __color.g, __color.b, __color.a]) }), text: ::std::option::Option::Some({ let __color: ::iced::Color = __ice_palette.colors[5]; ::ui_lang_guest::wire::Rgba([__color.r, __color.g, __color.b, __color.a]) }), border: ::std::option::Option::Some(::ui_lang_guest::wire::Border { color: ::std::option::Option::Some({ let __color: ::iced::Color = ::iced::Color::from_rgba8(0, 0, 0, 0.000000); ::ui_lang_guest::wire::Rgba([__color.r, __color.g, __color.b, __color.a]) }), width: ::std::option::Option::Some(((1.0) as f32).max(0.0).min(f32::MAX)), radius: ::std::option::Option::Some([((5.0) as f32).max(0.0).min(f32::MAX), ((5.0) as f32).max(0.0).min(f32::MAX), ((5.0) as f32).max(0.0).min(f32::MAX), ((5.0) as f32).max(0.0).min(f32::MAX)]) }) }, hovered: ::std::option::Option::Some(::ui_lang_guest::wire::Face { background: ::std::option::Option::Some({ let __color: ::iced::Color = __ice_palette.colors[60]; ::ui_lang_guest::wire::Rgba([__color.r, __color.g, __color.b, __color.a]) }), text: ::std::option::Option::Some({ let __color: ::iced::Color = __ice_palette.colors[4]; ::ui_lang_guest::wire::Rgba([__color.r, __color.g, __color.b, __color.a]) }), border: ::std::option::Option::None }), pressed: ::std::option::Option::Some(::ui_lang_guest::wire::Face { background: ::std::option::Option::Some({ let __color: ::iced::Color = __ice_palette.colors[56]; ::ui_lang_guest::wire::Rgba([__color.r, __color.g, __color.b, __color.a]) }), text: ::std::option::Option::Some({ let __color: ::iced::Color = __ice_palette.colors[4]; ::ui_lang_guest::wire::Rgba([__color.r, __color.g, __color.b, __color.a]) }), border: ::std::option::Option::None }), disabled: ::std::option::Option::None } };
#[cfg(test)]
drop(__ice_render_source);
__ice_rendered
}); } if self.page_create_open { __children.push({
#[cfg(test)]
let __ice_render_source_location = ::ui_lang_runtime::testing::Location::new("src/ui/pages.ice", 68, 1, "rendered view node");
#[cfg(test)]
let __ice_render_source = ::ui_lang_runtime::testing::push_render_source(__ice_render_source_location);
let __ice_rendered: __IceElement<'_, __PagesViewMessage> = ::ui_lang_guest::wire::Node::Button { checked: ::std::option::Option::None, expanded: ::std::option::Option::Some(self.page_create_open), description: ::std::option::Option::None, key: format!("{}/@button:68", __ice_use_scope), content: ::ui_lang_guest::wire::ButtonContent::Child(::std::boxed::Box::new({
#[cfg(test)]
let __ice_render_source_location = ::ui_lang_runtime::testing::Location::new("src/ui/pages.ice", 77, 1, "rendered view node");
#[cfg(test)]
let __ice_render_source = ::ui_lang_runtime::testing::push_render_source(__ice_render_source_location);
let __ice_rendered: __IceElement<'_, __PagesViewMessage> = ::ui_lang_guest::wire::Node::Container { shadow: ::ui_lang_guest::wire::Shadow { color: ::std::option::Option::None, x: ::std::option::Option::None, y: ::std::option::Option::None, blur: ::std::option::Option::None }, max_width: ::std::option::Option::None, max_height: ::std::option::Option::None, clip: false, key: format!("{}/@container:77", __ice_use_scope), width: ::std::option::Option::Some(::ui_lang_guest::wire::Length::Fill), height: ::std::option::Option::Some(::ui_lang_guest::wire::Length::Fill), padding: ::std::option::Option::None, align_x: ::std::option::Option::Some(::ui_lang_guest::wire::AlignX::Center), align_y: ::std::option::Option::Some(::ui_lang_guest::wire::AlignY::Center), background: (::std::option::Option::None).map(::ui_lang_guest::wire::Background::Color), border: ::std::option::Option::None, snap: ::std::option::Option::None, content: ::std::boxed::Box::new({
#[cfg(test)]
let __ice_render_source_location = ::ui_lang_runtime::testing::Location::new("src/ui/pages.ice", 83, 1, "rendered view node");
#[cfg(test)]
let __ice_render_source = ::ui_lang_runtime::testing::push_render_source(__ice_render_source_location);
let __ice_rendered: __IceElement<'_, __PagesViewMessage> = ::ui_lang_guest::wire::Node::Text { options: ::ui_lang_guest::wire::TextOptions { height: ::std::option::Option::None, align_y: ::std::option::Option::None, line_height: ::std::option::Option::None, shaping: ::std::option::Option::None, wrapping: ::std::option::Option::Some(::ui_lang_guest::wire::Wrapping::None), tracking: 0.0f32, font: ::std::option::Option::Some(::ui_lang_guest::wire::NamedFont { family: ::ui_lang_guest::wire::FontFamily::Named("Geist".into()), weight: ::ui_lang_guest::wire::Weight::Normal, stretch: ::ui_lang_guest::wire::FontStretch::Normal, style: ::ui_lang_guest::wire::FontStyle::Normal }) }, key: format!("{}/@text:83", __ice_use_scope), size: ::std::option::Option::Some(((13.0) as f32).max(f32::EPSILON).min(f32::MAX)), color: ::std::option::Option::Some({ let __color: ::iced::Color = __ice_palette.colors[5]; ::ui_lang_guest::wire::Rgba([__color.r, __color.g, __color.b, __color.a]) }), font: ::ui_lang_guest::wire::Font { monospace: false, weight: ::ui_lang_guest::wire::Weight::Normal }, width: ::std::option::Option::None, align_x: ::std::option::Option::None, content: ("×".to_owned()).to_string() };
#[cfg(test)]
drop(__ice_render_source);
__ice_rendered
}) };
#[cfg(test)]
drop(__ice_render_source);
__ice_rendered
})), label: ::std::option::Option::Some(::std::string::String::from("Close new page".to_owned())), on_press: if ((self.loading || (self.busy || (!(self.host_error).is_empty())))) { ::std::option::Option::None } else { ::std::option::Option::Some(::ui_lang_guest::slots::message((__ice_cb_21)())) }, width: ::std::option::Option::Some(::ui_lang_guest::wire::Length::Fixed((24.0) as f32)), height: ::std::option::Option::Some(::ui_lang_guest::wire::Length::Fixed((24.0) as f32)), padding: ::std::option::Option::Some(::ui_lang_guest::wire::Edges::all((0.0) as f32)), style: ::ui_lang_guest::wire::ButtonStyle { preset: ::ui_lang_guest::wire::ButtonPreset::Primary, recipe: Some(::ui_lang_guest::wire::ButtonRecipe { base: ::ui_lang_guest::wire::Face { background: ::std::option::Option::Some({ let __color: ::iced::Color = ::iced::Color::from_rgba8(0, 0, 0, 0.000000); ::ui_lang_guest::wire::Rgba([__color.r, __color.g, __color.b, __color.a]) }), text: ::std::option::Option::Some({ let __color: ::iced::Color = __ice_palette.colors[4]; ::ui_lang_guest::wire::Rgba([__color.r, __color.g, __color.b, __color.a]) }), border: ::std::option::Option::Some(::ui_lang_guest::wire::Border { color: ::std::option::Option::None, width: ::std::option::Option::None, radius: ::std::option::Option::Some([7.0; 4]) }) }, hover_background: ::std::option::Option::Some({ let __color: ::iced::Color = __ice_palette.colors[14]; ::ui_lang_guest::wire::Rgba([__color.r, __color.g, __color.b, __color.a]) }), pressed_background: ::std::option::Option::Some({ let __color: ::iced::Color = __ice_palette.colors[39]; ::ui_lang_guest::wire::Rgba([__color.r, __color.g, __color.b, __color.a]) }), disabled_background: ::std::option::Option::None, disabled_text: ::std::option::Option::None, disabled_opacity: ::std::option::Option::Some(0.5f32), focus_ring: ::std::option::Option::Some({ let __color: ::iced::Color = __ice_palette.colors[42]; ::ui_lang_guest::wire::Rgba([__color.r, __color.g, __color.b, __color.a]) }), text_size: ::std::option::Option::Some(13.5f32), line_height: ::std::option::Option::None, font: Some(::ui_lang_guest::wire::NamedFont { family: ::ui_lang_guest::wire::FontFamily::Named("Geist".into()), weight: ::ui_lang_guest::wire::Weight::Normal, stretch: ::ui_lang_guest::wire::FontStretch::Normal, style: ::ui_lang_guest::wire::FontStyle::Normal }) }), active: ::ui_lang_guest::wire::Face { background: ::std::option::Option::Some({ let __color: ::iced::Color = __ice_palette.colors[60]; ::ui_lang_guest::wire::Rgba([__color.r, __color.g, __color.b, __color.a]) }), text: ::std::option::Option::Some({ let __color: ::iced::Color = __ice_palette.colors[5]; ::ui_lang_guest::wire::Rgba([__color.r, __color.g, __color.b, __color.a]) }), border: ::std::option::Option::Some(::ui_lang_guest::wire::Border { color: ::std::option::Option::Some({ let __color: ::iced::Color = ::iced::Color::from_rgba8(0, 0, 0, 0.000000); ::ui_lang_guest::wire::Rgba([__color.r, __color.g, __color.b, __color.a]) }), width: ::std::option::Option::Some(((1.0) as f32).max(0.0).min(f32::MAX)), radius: ::std::option::Option::Some([((5.0) as f32).max(0.0).min(f32::MAX), ((5.0) as f32).max(0.0).min(f32::MAX), ((5.0) as f32).max(0.0).min(f32::MAX), ((5.0) as f32).max(0.0).min(f32::MAX)]) }) }, hovered: ::std::option::Option::Some(::ui_lang_guest::wire::Face { background: ::std::option::Option::Some({ let __color: ::iced::Color = __ice_palette.colors[56]; ::ui_lang_guest::wire::Rgba([__color.r, __color.g, __color.b, __color.a]) }), text: ::std::option::Option::Some({ let __color: ::iced::Color = __ice_palette.colors[4]; ::ui_lang_guest::wire::Rgba([__color.r, __color.g, __color.b, __color.a]) }), border: ::std::option::Option::None }), pressed: ::std::option::Option::Some(::ui_lang_guest::wire::Face { background: ::std::option::Option::Some({ let __color: ::iced::Color = __ice_palette.colors[56]; ::ui_lang_guest::wire::Rgba([__color.r, __color.g, __color.b, __color.a]) }), text: ::std::option::Option::Some({ let __color: ::iced::Color = __ice_palette.colors[4]; ::ui_lang_guest::wire::Rgba([__color.r, __color.g, __color.b, __color.a]) }), border: ::std::option::Option::None }), disabled: ::std::option::Option::None } };
#[cfg(test)]
drop(__ice_render_source);
__ice_rendered
}); } ::ui_lang_guest::wire::Node::Linear { max_width: ::std::option::Option::None, clip: false, key: format!("{}/@layout:50", __ice_use_scope), wrap: None, axis: ::ui_lang_guest::wire::Axis::Column, spacing: ::std::option::Option::None, padding: ::std::option::Option::None, width: ::std::option::Option::None, height: ::std::option::Option::None, align: ::std::option::Option::None, background: ::std::option::Option::None, border: ::std::option::Option::None, children: __children } };
#[cfg(test)]
drop(__ice_render_source);
__ice_rendered
}; __slot_content })();
#[cfg(test)]
drop(__ice_render_source);
__ice_rendered
}); ::ui_lang_guest::wire::Node::Linear { max_width: ::std::option::Option::None, clip: false, key: format!("{}/@layout:13", __ice_use_scope), wrap: None, axis: ::ui_lang_guest::wire::Axis::Row, spacing: ::std::option::Option::Some((8.0) as f32), padding: ::std::option::Option::None, width: ::std::option::Option::Some(::ui_lang_guest::wire::Length::Fill), height: ::std::option::Option::Some(::ui_lang_guest::wire::Length::Fill), align: ::std::option::Option::Some(::ui_lang_guest::wire::AlignX::Center), background: ::std::option::Option::None, border: ::std::option::Option::None, children: __children } };
#[cfg(test)]
drop(__ice_render_source);
__ice_rendered
}) };
#[cfg(test)]
drop(__ice_render_source);
__ice_rendered
}); __children.push({
#[cfg(test)]
let __ice_render_source_location = ::ui_lang_runtime::testing::Location::new("src/ui/kit.ice", 33, 1, "rendered view node");
#[cfg(test)]
let __ice_render_source = ::ui_lang_runtime::testing::push_render_source(__ice_render_source_location);
let __ice_rendered: __IceElement<'_, __PagesViewMessage> = ::ui_lang_guest::wire::Node::Container { shadow: ::ui_lang_guest::wire::Shadow { color: ::std::option::Option::None, x: ::std::option::Option::None, y: ::std::option::Option::None, blur: ::std::option::Option::None }, max_width: ::std::option::Option::None, max_height: ::std::option::Option::None, clip: false, key: format!("{}/@container:33", __ice_use_scope), width: ::std::option::Option::Some(::ui_lang_guest::wire::Length::Fill), height: ::std::option::Option::Some(::ui_lang_guest::wire::Length::Fixed((1.0) as f32)), padding: ::std::option::Option::None, align_x: ::std::option::Option::None, align_y: ::std::option::Option::None, background: (::std::option::Option::Some({ let __color: ::iced::Color = __ice_palette.colors[60]; ::ui_lang_guest::wire::Rgba([__color.r, __color.g, __color.b, __color.a]) })).map(::ui_lang_guest::wire::Background::Color), border: ::std::option::Option::None, snap: ::std::option::Option::None, content: ::std::boxed::Box::new({
#[cfg(test)]
let __ice_render_source_location = ::ui_lang_runtime::testing::Location::new("src/ui/kit.ice", 38, 1, "rendered view node");
#[cfg(test)]
let __ice_render_source = ::ui_lang_runtime::testing::push_render_source(__ice_render_source_location);
let __ice_rendered: __IceElement<'_, __PagesViewMessage> = ::ui_lang_guest::wire::Node::Space { width: ::std::option::Option::Some(::ui_lang_guest::wire::Length::Fixed((1.0) as f32)), height: ::std::option::Option::Some(::ui_lang_guest::wire::Length::Fixed((1.0) as f32)) };
#[cfg(test)]
drop(__ice_render_source);
__ice_rendered
}) };
#[cfg(test)]
drop(__ice_render_source);
__ice_rendered
}); ::ui_lang_guest::wire::Node::Linear { max_width: ::std::option::Option::None, clip: false, key: __ice_node_scope.clone(), wrap: None, axis: ::ui_lang_guest::wire::Axis::Column, spacing: ::std::option::Option::None, padding: ::std::option::Option::None, width: ::std::option::Option::Some(::ui_lang_guest::wire::Length::Fill), height: ::std::option::Option::None, align: ::std::option::Option::None, background: ::std::option::Option::None, border: ::std::option::Option::None, children: __children } } };
#[cfg(test)]
drop(__ice_render_source);
__ice_rendered
}; __component_content }
pub(super) fn __ice_component_use_6(&self, __ice_palette: __IcePalette, __ice_use_scope: ::std::string::String) -> __IceElement<'_, __PagesViewMessage> { let __component_content: __IceElement<'_, __PagesViewMessage> = {
#[cfg(test)]
let __ice_render_source_location = ::ui_lang_runtime::testing::Location::new("src/ui/kit.ice", 41, 1, "rendered view node");
#[cfg(test)]
let __ice_render_source = ::ui_lang_runtime::testing::push_render_source(__ice_render_source_location);
let __ice_rendered: __IceElement<'_, __PagesViewMessage> = { let __ice_node_scope = format!("{}/root", __ice_use_scope); ::ui_lang_guest::wire::Node::Container { shadow: ::ui_lang_guest::wire::Shadow { color: ::std::option::Option::None, x: ::std::option::Option::None, y: ::std::option::Option::None, blur: ::std::option::Option::None }, max_width: ::std::option::Option::None, max_height: ::std::option::Option::None, clip: false, key: __ice_node_scope.clone(), width: ::std::option::Option::Some(::ui_lang_guest::wire::Length::Fill), height: ::std::option::Option::Some(::ui_lang_guest::wire::Length::Fill), padding: ::std::option::Option::Some(::ui_lang_guest::wire::Edges { top: (22.0) as f32, right: (22.0) as f32, bottom: (22.0) as f32, left: (22.0) as f32 }), align_x: ::std::option::Option::Some(::ui_lang_guest::wire::AlignX::Center), align_y: ::std::option::Option::Some(::ui_lang_guest::wire::AlignY::Center), background: (::std::option::Option::None).map(::ui_lang_guest::wire::Background::Color), border: ::std::option::Option::None, snap: ::std::option::Option::None, content: ::std::boxed::Box::new({
#[cfg(test)]
let __ice_render_source_location = ::ui_lang_runtime::testing::Location::new("src/ui/kit.ice", 48, 1, "rendered view node");
#[cfg(test)]
let __ice_render_source = ::ui_lang_runtime::testing::push_render_source(__ice_render_source_location);
let __ice_rendered: __IceElement<'_, __PagesViewMessage> = { let mut __children: ::std::vec::Vec<__IceElement<'_, __PagesViewMessage>> = ::std::vec::Vec::new(); __children.push({
#[cfg(test)]
let __ice_render_source_location = ::ui_lang_runtime::testing::Location::new("src/ui/kit.ice", 53, 1, "rendered view node");
#[cfg(test)]
let __ice_render_source = ::ui_lang_runtime::testing::push_render_source(__ice_render_source_location);
let __ice_rendered: __IceElement<'_, __PagesViewMessage> = ::ui_lang_guest::wire::Node::Container { shadow: ::ui_lang_guest::wire::Shadow { color: ::std::option::Option::None, x: ::std::option::Option::None, y: ::std::option::Option::None, blur: ::std::option::Option::None }, max_width: ::std::option::Option::None, max_height: ::std::option::Option::None, clip: false, key: format!("{}/@container:53", __ice_use_scope), width: ::std::option::Option::Some(::ui_lang_guest::wire::Length::Fixed((42.0) as f32)), height: ::std::option::Option::Some(::ui_lang_guest::wire::Length::Fixed((42.0) as f32)), padding: ::std::option::Option::None, align_x: ::std::option::Option::Some(::ui_lang_guest::wire::AlignX::Center), align_y: ::std::option::Option::Some(::ui_lang_guest::wire::AlignY::Center), background: (::std::option::Option::Some({ let __color: ::iced::Color = __ice_palette.colors[3]; ::ui_lang_guest::wire::Rgba([__color.r, __color.g, __color.b, __color.a]) })).map(::ui_lang_guest::wire::Background::Color), border: ::std::option::Option::Some(::ui_lang_guest::wire::Border { color: ::std::option::Option::Some({ let __color: ::iced::Color = __ice_palette.colors[39]; ::ui_lang_guest::wire::Rgba([__color.r, __color.g, __color.b, __color.a]) }), width: ::std::option::Option::Some(((1.0) as f32).max(0.0).min(f32::MAX)), radius: ::std::option::Option::Some([((21.0) as f32).max(0.0).min(f32::MAX), ((21.0) as f32).max(0.0).min(f32::MAX), ((21.0) as f32).max(0.0).min(f32::MAX), ((21.0) as f32).max(0.0).min(f32::MAX)]) }), snap: ::std::option::Option::None, content: ::std::boxed::Box::new({
#[cfg(test)]
let __ice_render_source_location = ::ui_lang_runtime::testing::Location::new("src/ui/kit.ice", 63, 1, "rendered view node");
#[cfg(test)]
let __ice_render_source = ::ui_lang_runtime::testing::push_render_source(__ice_render_source_location);
let __ice_rendered: __IceElement<'_, __PagesViewMessage> = ::ui_lang_guest::wire::Node::Text { options: ::ui_lang_guest::wire::TextOptions { height: ::std::option::Option::None, align_y: ::std::option::Option::None, line_height: ::std::option::Option::None, shaping: ::std::option::Option::None, wrapping: ::std::option::Option::None, tracking: 0.0f32, font: ::std::option::Option::Some(::ui_lang_guest::wire::NamedFont { family: ::ui_lang_guest::wire::FontFamily::Named("Geist".into()), weight: ::ui_lang_guest::wire::Weight::Normal, stretch: ::ui_lang_guest::wire::FontStretch::Normal, style: ::ui_lang_guest::wire::FontStyle::Normal }) }, key: format!("{}/@text:63", __ice_use_scope), size: ::std::option::Option::Some(((20.0) as f32).max(f32::EPSILON).min(f32::MAX)), color: ::std::option::Option::Some({ let __color: ::iced::Color = __ice_palette.colors[7]; ::ui_lang_guest::wire::Rgba([__color.r, __color.g, __color.b, __color.a]) }), font: ::ui_lang_guest::wire::Font { monospace: false, weight: ::ui_lang_guest::wire::Weight::Normal }, width: ::std::option::Option::None, align_x: ::std::option::Option::None, content: ("◇".to_owned()).to_string() };
#[cfg(test)]
drop(__ice_render_source);
__ice_rendered
}) };
#[cfg(test)]
drop(__ice_render_source);
__ice_rendered
}); __children.push({
#[cfg(test)]
let __ice_render_source_location = ::ui_lang_runtime::testing::Location::new("src/ui/kit.ice", 64, 1, "rendered view node");
#[cfg(test)]
let __ice_render_source = ::ui_lang_runtime::testing::push_render_source(__ice_render_source_location);
let __ice_rendered: __IceElement<'_, __PagesViewMessage> = ::ui_lang_guest::wire::Node::Text { options: ::ui_lang_guest::wire::TextOptions { height: ::std::option::Option::None, align_y: ::std::option::Option::None, line_height: ::std::option::Option::Some(::ui_lang_guest::wire::LineHeight::Relative(1.35f32)), shaping: ::std::option::Option::None, wrapping: ::std::option::Option::None, tracking: 0.0f32, font: ::std::option::Option::Some(::ui_lang_guest::wire::NamedFont { family: ::ui_lang_guest::wire::FontFamily::Named("Geist".into()), weight: ::ui_lang_guest::wire::Weight::Semibold, stretch: ::ui_lang_guest::wire::FontStretch::Normal, style: ::ui_lang_guest::wire::FontStyle::Normal }) }, key: format!("{}/@text:64", __ice_use_scope), size: ::std::option::Option::Some(16.0f32), color: ::std::option::Option::Some({ let __color: ::iced::Color = __ice_palette.colors[4]; ::ui_lang_guest::wire::Rgba([__color.r, __color.g, __color.b, __color.a]) }), font: ::ui_lang_guest::wire::Font { monospace: false, weight: ::ui_lang_guest::wire::Weight::Semibold }, width: ::std::option::Option::None, align_x: ::std::option::Option::None, content: ("Not connected".to_owned()).to_string() };
#[cfg(test)]
drop(__ice_render_source);
__ice_rendered
}); __children.push({
#[cfg(test)]
let __ice_render_source_location = ::ui_lang_runtime::testing::Location::new("src/ui/kit.ice", 65, 1, "rendered view node");
#[cfg(test)]
let __ice_render_source = ::ui_lang_runtime::testing::push_render_source(__ice_render_source_location);
let __ice_rendered: __IceElement<'_, __PagesViewMessage> = ::ui_lang_guest::wire::Node::Text { options: ::ui_lang_guest::wire::TextOptions { height: ::std::option::Option::None, align_y: ::std::option::Option::None, line_height: ::std::option::Option::Some(::ui_lang_guest::wire::LineHeight::Relative(1.5f32)), shaping: ::std::option::Option::None, wrapping: ::std::option::Option::None, tracking: 0.0f32, font: ::std::option::Option::Some(::ui_lang_guest::wire::NamedFont { family: ::ui_lang_guest::wire::FontFamily::Named("Geist".into()), weight: ::ui_lang_guest::wire::Weight::Normal, stretch: ::ui_lang_guest::wire::FontStretch::Normal, style: ::ui_lang_guest::wire::FontStyle::Normal }) }, key: format!("{}/@text:65", __ice_use_scope), size: ::std::option::Option::Some(12.5f32), color: ::std::option::Option::Some({ let __color: ::iced::Color = __ice_palette.colors[5]; ::ui_lang_guest::wire::Rgba([__color.r, __color.g, __color.b, __color.a]) }), font: ::ui_lang_guest::wire::Font { monospace: false, weight: ::ui_lang_guest::wire::Weight::Normal }, width: ::std::option::Option::None, align_x: ::std::option::Option::None, content: ("Click the network name in the titlebar to pick or reconnect a network.".to_owned()).to_string() };
#[cfg(test)]
drop(__ice_render_source);
__ice_rendered
}); ::ui_lang_guest::wire::Node::Linear { max_width: ::std::option::Option::None, clip: false, key: format!("{}/@layout:48", __ice_use_scope), wrap: None, axis: ::ui_lang_guest::wire::Axis::Column, spacing: ::std::option::Option::Some((7.0) as f32), padding: ::std::option::Option::None, width: ::std::option::Option::Some(::ui_lang_guest::wire::Length::Fill), height: ::std::option::Option::None, align: ::std::option::Option::Some(::ui_lang_guest::wire::AlignX::Center), background: ::std::option::Option::None, border: ::std::option::Option::None, children: __children } };
#[cfg(test)]
drop(__ice_render_source);
__ice_rendered
}) } };
#[cfg(test)]
drop(__ice_render_source);
__ice_rendered
}; __component_content }
pub(super) fn __ice_component_use_7(&self, __ice_palette: __IcePalette, __ice_use_scope: ::std::string::String) -> __IceElement<'_, __PagesViewMessage> { let __component_content: __IceElement<'_, __PagesViewMessage> = {
#[cfg(test)]
let __ice_render_source_location = ::ui_lang_runtime::testing::Location::new("src/ui/kit.ice", 41, 1, "rendered view node");
#[cfg(test)]
let __ice_render_source = ::ui_lang_runtime::testing::push_render_source(__ice_render_source_location);
let __ice_rendered: __IceElement<'_, __PagesViewMessage> = { let __ice_node_scope = format!("{}/root", __ice_use_scope); ::ui_lang_guest::wire::Node::Container { shadow: ::ui_lang_guest::wire::Shadow { color: ::std::option::Option::None, x: ::std::option::Option::None, y: ::std::option::Option::None, blur: ::std::option::Option::None }, max_width: ::std::option::Option::None, max_height: ::std::option::Option::None, clip: false, key: __ice_node_scope.clone(), width: ::std::option::Option::Some(::ui_lang_guest::wire::Length::Fill), height: ::std::option::Option::Some(::ui_lang_guest::wire::Length::Fill), padding: ::std::option::Option::Some(::ui_lang_guest::wire::Edges { top: (22.0) as f32, right: (22.0) as f32, bottom: (22.0) as f32, left: (22.0) as f32 }), align_x: ::std::option::Option::Some(::ui_lang_guest::wire::AlignX::Center), align_y: ::std::option::Option::Some(::ui_lang_guest::wire::AlignY::Center), background: (::std::option::Option::None).map(::ui_lang_guest::wire::Background::Color), border: ::std::option::Option::None, snap: ::std::option::Option::None, content: ::std::boxed::Box::new({
#[cfg(test)]
let __ice_render_source_location = ::ui_lang_runtime::testing::Location::new("src/ui/kit.ice", 48, 1, "rendered view node");
#[cfg(test)]
let __ice_render_source = ::ui_lang_runtime::testing::push_render_source(__ice_render_source_location);
let __ice_rendered: __IceElement<'_, __PagesViewMessage> = { let mut __children: ::std::vec::Vec<__IceElement<'_, __PagesViewMessage>> = ::std::vec::Vec::new(); __children.push({
#[cfg(test)]
let __ice_render_source_location = ::ui_lang_runtime::testing::Location::new("src/ui/kit.ice", 53, 1, "rendered view node");
#[cfg(test)]
let __ice_render_source = ::ui_lang_runtime::testing::push_render_source(__ice_render_source_location);
let __ice_rendered: __IceElement<'_, __PagesViewMessage> = ::ui_lang_guest::wire::Node::Container { shadow: ::ui_lang_guest::wire::Shadow { color: ::std::option::Option::None, x: ::std::option::Option::None, y: ::std::option::Option::None, blur: ::std::option::Option::None }, max_width: ::std::option::Option::None, max_height: ::std::option::Option::None, clip: false, key: format!("{}/@container:53", __ice_use_scope), width: ::std::option::Option::Some(::ui_lang_guest::wire::Length::Fixed((42.0) as f32)), height: ::std::option::Option::Some(::ui_lang_guest::wire::Length::Fixed((42.0) as f32)), padding: ::std::option::Option::None, align_x: ::std::option::Option::Some(::ui_lang_guest::wire::AlignX::Center), align_y: ::std::option::Option::Some(::ui_lang_guest::wire::AlignY::Center), background: (::std::option::Option::Some({ let __color: ::iced::Color = __ice_palette.colors[3]; ::ui_lang_guest::wire::Rgba([__color.r, __color.g, __color.b, __color.a]) })).map(::ui_lang_guest::wire::Background::Color), border: ::std::option::Option::Some(::ui_lang_guest::wire::Border { color: ::std::option::Option::Some({ let __color: ::iced::Color = __ice_palette.colors[39]; ::ui_lang_guest::wire::Rgba([__color.r, __color.g, __color.b, __color.a]) }), width: ::std::option::Option::Some(((1.0) as f32).max(0.0).min(f32::MAX)), radius: ::std::option::Option::Some([((21.0) as f32).max(0.0).min(f32::MAX), ((21.0) as f32).max(0.0).min(f32::MAX), ((21.0) as f32).max(0.0).min(f32::MAX), ((21.0) as f32).max(0.0).min(f32::MAX)]) }), snap: ::std::option::Option::None, content: ::std::boxed::Box::new({
#[cfg(test)]
let __ice_render_source_location = ::ui_lang_runtime::testing::Location::new("src/ui/kit.ice", 63, 1, "rendered view node");
#[cfg(test)]
let __ice_render_source = ::ui_lang_runtime::testing::push_render_source(__ice_render_source_location);
let __ice_rendered: __IceElement<'_, __PagesViewMessage> = ::ui_lang_guest::wire::Node::Text { options: ::ui_lang_guest::wire::TextOptions { height: ::std::option::Option::None, align_y: ::std::option::Option::None, line_height: ::std::option::Option::None, shaping: ::std::option::Option::None, wrapping: ::std::option::Option::None, tracking: 0.0f32, font: ::std::option::Option::Some(::ui_lang_guest::wire::NamedFont { family: ::ui_lang_guest::wire::FontFamily::Named("Geist".into()), weight: ::ui_lang_guest::wire::Weight::Normal, stretch: ::ui_lang_guest::wire::FontStretch::Normal, style: ::ui_lang_guest::wire::FontStyle::Normal }) }, key: format!("{}/@text:63", __ice_use_scope), size: ::std::option::Option::Some(((20.0) as f32).max(f32::EPSILON).min(f32::MAX)), color: ::std::option::Option::Some({ let __color: ::iced::Color = __ice_palette.colors[7]; ::ui_lang_guest::wire::Rgba([__color.r, __color.g, __color.b, __color.a]) }), font: ::ui_lang_guest::wire::Font { monospace: false, weight: ::ui_lang_guest::wire::Weight::Normal }, width: ::std::option::Option::None, align_x: ::std::option::Option::None, content: ("◇".to_owned()).to_string() };
#[cfg(test)]
drop(__ice_render_source);
__ice_rendered
}) };
#[cfg(test)]
drop(__ice_render_source);
__ice_rendered
}); __children.push({
#[cfg(test)]
let __ice_render_source_location = ::ui_lang_runtime::testing::Location::new("src/ui/kit.ice", 64, 1, "rendered view node");
#[cfg(test)]
let __ice_render_source = ::ui_lang_runtime::testing::push_render_source(__ice_render_source_location);
let __ice_rendered: __IceElement<'_, __PagesViewMessage> = ::ui_lang_guest::wire::Node::Text { options: ::ui_lang_guest::wire::TextOptions { height: ::std::option::Option::None, align_y: ::std::option::Option::None, line_height: ::std::option::Option::Some(::ui_lang_guest::wire::LineHeight::Relative(1.35f32)), shaping: ::std::option::Option::None, wrapping: ::std::option::Option::None, tracking: 0.0f32, font: ::std::option::Option::Some(::ui_lang_guest::wire::NamedFont { family: ::ui_lang_guest::wire::FontFamily::Named("Geist".into()), weight: ::ui_lang_guest::wire::Weight::Semibold, stretch: ::ui_lang_guest::wire::FontStretch::Normal, style: ::ui_lang_guest::wire::FontStyle::Normal }) }, key: format!("{}/@text:64", __ice_use_scope), size: ::std::option::Option::Some(16.0f32), color: ::std::option::Option::Some({ let __color: ::iced::Color = __ice_palette.colors[4]; ::ui_lang_guest::wire::Rgba([__color.r, __color.g, __color.b, __color.a]) }), font: ::ui_lang_guest::wire::Font { monospace: false, weight: ::ui_lang_guest::wire::Weight::Semibold }, width: ::std::option::Option::None, align_x: ::std::option::Option::None, content: ("Loading pages…".to_owned()).to_string() };
#[cfg(test)]
drop(__ice_render_source);
__ice_rendered
}); __children.push({
#[cfg(test)]
let __ice_render_source_location = ::ui_lang_runtime::testing::Location::new("src/ui/kit.ice", 65, 1, "rendered view node");
#[cfg(test)]
let __ice_render_source = ::ui_lang_runtime::testing::push_render_source(__ice_render_source_location);
let __ice_rendered: __IceElement<'_, __PagesViewMessage> = ::ui_lang_guest::wire::Node::Text { options: ::ui_lang_guest::wire::TextOptions { height: ::std::option::Option::None, align_y: ::std::option::Option::None, line_height: ::std::option::Option::Some(::ui_lang_guest::wire::LineHeight::Relative(1.5f32)), shaping: ::std::option::Option::None, wrapping: ::std::option::Option::None, tracking: 0.0f32, font: ::std::option::Option::Some(::ui_lang_guest::wire::NamedFont { family: ::ui_lang_guest::wire::FontFamily::Named("Geist".into()), weight: ::ui_lang_guest::wire::Weight::Normal, stretch: ::ui_lang_guest::wire::FontStretch::Normal, style: ::ui_lang_guest::wire::FontStyle::Normal }) }, key: format!("{}/@text:65", __ice_use_scope), size: ::std::option::Option::Some(12.5f32), color: ::std::option::Option::Some({ let __color: ::iced::Color = __ice_palette.colors[5]; ::ui_lang_guest::wire::Rgba([__color.r, __color.g, __color.b, __color.a]) }), font: ::ui_lang_guest::wire::Font { monospace: false, weight: ::ui_lang_guest::wire::Weight::Normal }, width: ::std::option::Option::None, align_x: ::std::option::Option::None, content: ("Waiting for the page list.".to_owned()).to_string() };
#[cfg(test)]
drop(__ice_render_source);
__ice_rendered
}); ::ui_lang_guest::wire::Node::Linear { max_width: ::std::option::Option::None, clip: false, key: format!("{}/@layout:48", __ice_use_scope), wrap: None, axis: ::ui_lang_guest::wire::Axis::Column, spacing: ::std::option::Option::Some((7.0) as f32), padding: ::std::option::Option::None, width: ::std::option::Option::Some(::ui_lang_guest::wire::Length::Fill), height: ::std::option::Option::None, align: ::std::option::Option::Some(::ui_lang_guest::wire::AlignX::Center), background: ::std::option::Option::None, border: ::std::option::Option::None, children: __children } };
#[cfg(test)]
drop(__ice_render_source);
__ice_rendered
}) } };
#[cfg(test)]
drop(__ice_render_source);
__ice_rendered
}; __component_content }
pub(super) fn __ice_component_use_8(&self, __ice_palette: __IcePalette, __ice_use_scope: ::std::string::String) -> __IceElement<'_, __PagesViewMessage> { let __component_content: __IceElement<'_, __PagesViewMessage> = {
#[cfg(test)]
let __ice_render_source_location = ::ui_lang_runtime::testing::Location::new("src/ui/kit.ice", 41, 1, "rendered view node");
#[cfg(test)]
let __ice_render_source = ::ui_lang_runtime::testing::push_render_source(__ice_render_source_location);
let __ice_rendered: __IceElement<'_, __PagesViewMessage> = { let __ice_node_scope = format!("{}/root", __ice_use_scope); ::ui_lang_guest::wire::Node::Container { shadow: ::ui_lang_guest::wire::Shadow { color: ::std::option::Option::None, x: ::std::option::Option::None, y: ::std::option::Option::None, blur: ::std::option::Option::None }, max_width: ::std::option::Option::None, max_height: ::std::option::Option::None, clip: false, key: __ice_node_scope.clone(), width: ::std::option::Option::Some(::ui_lang_guest::wire::Length::Fill), height: ::std::option::Option::Some(::ui_lang_guest::wire::Length::Fill), padding: ::std::option::Option::Some(::ui_lang_guest::wire::Edges { top: (22.0) as f32, right: (22.0) as f32, bottom: (22.0) as f32, left: (22.0) as f32 }), align_x: ::std::option::Option::Some(::ui_lang_guest::wire::AlignX::Center), align_y: ::std::option::Option::Some(::ui_lang_guest::wire::AlignY::Center), background: (::std::option::Option::None).map(::ui_lang_guest::wire::Background::Color), border: ::std::option::Option::None, snap: ::std::option::Option::None, content: ::std::boxed::Box::new({
#[cfg(test)]
let __ice_render_source_location = ::ui_lang_runtime::testing::Location::new("src/ui/kit.ice", 48, 1, "rendered view node");
#[cfg(test)]
let __ice_render_source = ::ui_lang_runtime::testing::push_render_source(__ice_render_source_location);
let __ice_rendered: __IceElement<'_, __PagesViewMessage> = { let mut __children: ::std::vec::Vec<__IceElement<'_, __PagesViewMessage>> = ::std::vec::Vec::new(); __children.push({
#[cfg(test)]
let __ice_render_source_location = ::ui_lang_runtime::testing::Location::new("src/ui/kit.ice", 53, 1, "rendered view node");
#[cfg(test)]
let __ice_render_source = ::ui_lang_runtime::testing::push_render_source(__ice_render_source_location);
let __ice_rendered: __IceElement<'_, __PagesViewMessage> = ::ui_lang_guest::wire::Node::Container { shadow: ::ui_lang_guest::wire::Shadow { color: ::std::option::Option::None, x: ::std::option::Option::None, y: ::std::option::Option::None, blur: ::std::option::Option::None }, max_width: ::std::option::Option::None, max_height: ::std::option::Option::None, clip: false, key: format!("{}/@container:53", __ice_use_scope), width: ::std::option::Option::Some(::ui_lang_guest::wire::Length::Fixed((42.0) as f32)), height: ::std::option::Option::Some(::ui_lang_guest::wire::Length::Fixed((42.0) as f32)), padding: ::std::option::Option::None, align_x: ::std::option::Option::Some(::ui_lang_guest::wire::AlignX::Center), align_y: ::std::option::Option::Some(::ui_lang_guest::wire::AlignY::Center), background: (::std::option::Option::Some({ let __color: ::iced::Color = __ice_palette.colors[3]; ::ui_lang_guest::wire::Rgba([__color.r, __color.g, __color.b, __color.a]) })).map(::ui_lang_guest::wire::Background::Color), border: ::std::option::Option::Some(::ui_lang_guest::wire::Border { color: ::std::option::Option::Some({ let __color: ::iced::Color = __ice_palette.colors[39]; ::ui_lang_guest::wire::Rgba([__color.r, __color.g, __color.b, __color.a]) }), width: ::std::option::Option::Some(((1.0) as f32).max(0.0).min(f32::MAX)), radius: ::std::option::Option::Some([((21.0) as f32).max(0.0).min(f32::MAX), ((21.0) as f32).max(0.0).min(f32::MAX), ((21.0) as f32).max(0.0).min(f32::MAX), ((21.0) as f32).max(0.0).min(f32::MAX)]) }), snap: ::std::option::Option::None, content: ::std::boxed::Box::new({
#[cfg(test)]
let __ice_render_source_location = ::ui_lang_runtime::testing::Location::new("src/ui/kit.ice", 63, 1, "rendered view node");
#[cfg(test)]
let __ice_render_source = ::ui_lang_runtime::testing::push_render_source(__ice_render_source_location);
let __ice_rendered: __IceElement<'_, __PagesViewMessage> = ::ui_lang_guest::wire::Node::Text { options: ::ui_lang_guest::wire::TextOptions { height: ::std::option::Option::None, align_y: ::std::option::Option::None, line_height: ::std::option::Option::None, shaping: ::std::option::Option::None, wrapping: ::std::option::Option::None, tracking: 0.0f32, font: ::std::option::Option::Some(::ui_lang_guest::wire::NamedFont { family: ::ui_lang_guest::wire::FontFamily::Named("Geist".into()), weight: ::ui_lang_guest::wire::Weight::Normal, stretch: ::ui_lang_guest::wire::FontStretch::Normal, style: ::ui_lang_guest::wire::FontStyle::Normal }) }, key: format!("{}/@text:63", __ice_use_scope), size: ::std::option::Option::Some(((20.0) as f32).max(f32::EPSILON).min(f32::MAX)), color: ::std::option::Option::Some({ let __color: ::iced::Color = __ice_palette.colors[7]; ::ui_lang_guest::wire::Rgba([__color.r, __color.g, __color.b, __color.a]) }), font: ::ui_lang_guest::wire::Font { monospace: false, weight: ::ui_lang_guest::wire::Weight::Normal }, width: ::std::option::Option::None, align_x: ::std::option::Option::None, content: ("◇".to_owned()).to_string() };
#[cfg(test)]
drop(__ice_render_source);
__ice_rendered
}) };
#[cfg(test)]
drop(__ice_render_source);
__ice_rendered
}); __children.push({
#[cfg(test)]
let __ice_render_source_location = ::ui_lang_runtime::testing::Location::new("src/ui/kit.ice", 64, 1, "rendered view node");
#[cfg(test)]
let __ice_render_source = ::ui_lang_runtime::testing::push_render_source(__ice_render_source_location);
let __ice_rendered: __IceElement<'_, __PagesViewMessage> = ::ui_lang_guest::wire::Node::Text { options: ::ui_lang_guest::wire::TextOptions { height: ::std::option::Option::None, align_y: ::std::option::Option::None, line_height: ::std::option::Option::Some(::ui_lang_guest::wire::LineHeight::Relative(1.35f32)), shaping: ::std::option::Option::None, wrapping: ::std::option::Option::None, tracking: 0.0f32, font: ::std::option::Option::Some(::ui_lang_guest::wire::NamedFont { family: ::ui_lang_guest::wire::FontFamily::Named("Geist".into()), weight: ::ui_lang_guest::wire::Weight::Semibold, stretch: ::ui_lang_guest::wire::FontStretch::Normal, style: ::ui_lang_guest::wire::FontStyle::Normal }) }, key: format!("{}/@text:64", __ice_use_scope), size: ::std::option::Option::Some(16.0f32), color: ::std::option::Option::Some({ let __color: ::iced::Color = __ice_palette.colors[4]; ::ui_lang_guest::wire::Rgba([__color.r, __color.g, __color.b, __color.a]) }), font: ::ui_lang_guest::wire::Font { monospace: false, weight: ::ui_lang_guest::wire::Weight::Semibold }, width: ::std::option::Option::None, align_x: ::std::option::Option::None, content: ("No page selected".to_owned()).to_string() };
#[cfg(test)]
drop(__ice_render_source);
__ice_rendered
}); __children.push({
#[cfg(test)]
let __ice_render_source_location = ::ui_lang_runtime::testing::Location::new("src/ui/kit.ice", 65, 1, "rendered view node");
#[cfg(test)]
let __ice_render_source = ::ui_lang_runtime::testing::push_render_source(__ice_render_source_location);
let __ice_rendered: __IceElement<'_, __PagesViewMessage> = ::ui_lang_guest::wire::Node::Text { options: ::ui_lang_guest::wire::TextOptions { height: ::std::option::Option::None, align_y: ::std::option::Option::None, line_height: ::std::option::Option::Some(::ui_lang_guest::wire::LineHeight::Relative(1.5f32)), shaping: ::std::option::Option::None, wrapping: ::std::option::Option::None, tracking: 0.0f32, font: ::std::option::Option::Some(::ui_lang_guest::wire::NamedFont { family: ::ui_lang_guest::wire::FontFamily::Named("Geist".into()), weight: ::ui_lang_guest::wire::Weight::Normal, stretch: ::ui_lang_guest::wire::FontStretch::Normal, style: ::ui_lang_guest::wire::FontStyle::Normal }) }, key: format!("{}/@text:65", __ice_use_scope), size: ::std::option::Option::Some(12.5f32), color: ::std::option::Option::Some({ let __color: ::iced::Color = __ice_palette.colors[5]; ::ui_lang_guest::wire::Rgba([__color.r, __color.g, __color.b, __color.a]) }), font: ::ui_lang_guest::wire::Font { monospace: false, weight: ::ui_lang_guest::wire::Weight::Normal }, width: ::std::option::Option::None, align_x: ::std::option::Option::None, content: ("Create a page from the sidebar.".to_owned()).to_string() };
#[cfg(test)]
drop(__ice_render_source);
__ice_rendered
}); ::ui_lang_guest::wire::Node::Linear { max_width: ::std::option::Option::None, clip: false, key: format!("{}/@layout:48", __ice_use_scope), wrap: None, axis: ::ui_lang_guest::wire::Axis::Column, spacing: ::std::option::Option::Some((7.0) as f32), padding: ::std::option::Option::None, width: ::std::option::Option::Some(::ui_lang_guest::wire::Length::Fill), height: ::std::option::Option::None, align: ::std::option::Option::Some(::ui_lang_guest::wire::AlignX::Center), background: ::std::option::Option::None, border: ::std::option::Option::None, children: __children } };
#[cfg(test)]
drop(__ice_render_source);
__ice_rendered
}) } };
#[cfg(test)]
drop(__ice_render_source);
__ice_rendered
}; __component_content }
pub(super) fn __ice_component_use_10(&self, __ice_palette: __IcePalette, __ice_use_scope: ::std::string::String) -> __IceElement<'_, __PagesViewMessage> { let __component_content: __IceElement<'_, __PagesViewMessage> = {
#[cfg(test)]
let __ice_render_source_location = ::ui_lang_runtime::testing::Location::new("src/ui/kit.ice", 68, 1, "rendered view node");
#[cfg(test)]
let __ice_render_source = ::ui_lang_runtime::testing::push_render_source(__ice_render_source_location);
let __ice_rendered: __IceElement<'_, __PagesViewMessage> = { let __ice_node_scope = format!("{}/root", __ice_use_scope); ::ui_lang_guest::wire::Node::Container { shadow: ::ui_lang_guest::wire::Shadow { color: ::std::option::Option::None, x: ::std::option::Option::None, y: ::std::option::Option::None, blur: ::std::option::Option::None }, max_width: ::std::option::Option::None, max_height: ::std::option::Option::None, clip: false, key: __ice_node_scope.clone(), width: ::std::option::Option::Some(::ui_lang_guest::wire::Length::Fill), height: ::std::option::Option::None, padding: ::std::option::Option::Some(::ui_lang_guest::wire::Edges { top: (30.0) as f32, right: (30.0) as f32, bottom: (30.0) as f32, left: (30.0) as f32 }), align_x: ::std::option::Option::Some(::ui_lang_guest::wire::AlignX::Center), align_y: ::std::option::Option::None, background: (::std::option::Option::Some({ let __color: ::iced::Color = ::iced::Color::from_rgba8(0, 0, 0, 0.000000); ::ui_lang_guest::wire::Rgba([__color.r, __color.g, __color.b, __color.a]) })).map(::ui_lang_guest::wire::Background::Color), border: ::std::option::Option::Some(::ui_lang_guest::wire::Border { color: ::std::option::Option::Some({ let __color: ::iced::Color = __ice_palette.colors[39]; ::ui_lang_guest::wire::Rgba([__color.r, __color.g, __color.b, __color.a]) }), width: ::std::option::Option::Some(((1.0) as f32).max(0.0).min(f32::MAX)), radius: ::std::option::Option::Some([((12.0) as f32).max(0.0).min(f32::MAX), ((12.0) as f32).max(0.0).min(f32::MAX), ((12.0) as f32).max(0.0).min(f32::MAX), ((12.0) as f32).max(0.0).min(f32::MAX)]) }), snap: ::std::option::Option::None, content: ::std::boxed::Box::new({
#[cfg(test)]
let __ice_render_source_location = ::ui_lang_runtime::testing::Location::new("src/ui/kit.ice", 77, 1, "rendered view node");
#[cfg(test)]
let __ice_render_source = ::ui_lang_runtime::testing::push_render_source(__ice_render_source_location);
let __ice_rendered: __IceElement<'_, __PagesViewMessage> = ::ui_lang_guest::wire::Node::Text { options: ::ui_lang_guest::wire::TextOptions { height: ::std::option::Option::None, align_y: ::std::option::Option::None, line_height: ::std::option::Option::None, shaping: ::std::option::Option::None, wrapping: ::std::option::Option::None, tracking: 0.0f32, font: ::std::option::Option::Some(::ui_lang_guest::wire::NamedFont { family: ::ui_lang_guest::wire::FontFamily::Named("Geist".into()), weight: ::ui_lang_guest::wire::Weight::Normal, stretch: ::ui_lang_guest::wire::FontStretch::Normal, style: ::ui_lang_guest::wire::FontStyle::Normal }) }, key: format!("{}/@text:77", __ice_use_scope), size: ::std::option::Option::Some(((13.0) as f32).max(f32::EPSILON).min(f32::MAX)), color: ::std::option::Option::Some({ let __color: ::iced::Color = __ice_palette.colors[71]; ::ui_lang_guest::wire::Rgba([__color.r, __color.g, __color.b, __color.a]) }), font: ::ui_lang_guest::wire::Font { monospace: false, weight: ::ui_lang_guest::wire::Weight::Normal }, width: ::std::option::Option::None, align_x: ::std::option::Option::None, content: ("No pages matched that search.".to_owned()).to_string() };
#[cfg(test)]
drop(__ice_render_source);
__ice_rendered
}) } };
#[cfg(test)]
drop(__ice_render_source);
__ice_rendered
}; __component_content }
pub(super) fn __ice_component_use_11(&self, __ice_palette: __IcePalette, __ice_use_scope: ::std::string::String, __ice_arg_0: ::std::string::String) -> __IceElement<'_, __PagesViewMessage> { let __component_content: __IceElement<'_, __PagesViewMessage> = {
#[cfg(test)]
let __ice_render_source_location = ::ui_lang_runtime::testing::Location::new("src/ui/kit.ice", 224, 1, "rendered view node");
#[cfg(test)]
let __ice_render_source = ::ui_lang_runtime::testing::push_render_source(__ice_render_source_location);
let __ice_rendered: __IceElement<'_, __PagesViewMessage> = { let __ice_node_scope = format!("{}/root", __ice_use_scope); ::ui_lang_guest::wire::Node::Container { shadow: ::ui_lang_guest::wire::Shadow { color: ::std::option::Option::None, x: ::std::option::Option::None, y: ::std::option::Option::None, blur: ::std::option::Option::None }, max_width: ::std::option::Option::None, max_height: ::std::option::Option::None, clip: false, key: __ice_node_scope.clone(), width: ::std::option::Option::Some(::ui_lang_guest::wire::Length::Fixed((22.0) as f32)), height: ::std::option::Option::Some(::ui_lang_guest::wire::Length::Fixed((22.0) as f32)), padding: ::std::option::Option::None, align_x: ::std::option::Option::Some(::ui_lang_guest::wire::AlignX::Center), align_y: ::std::option::Option::Some(::ui_lang_guest::wire::AlignY::Center), background: (::std::option::Option::Some({ let __color: ::iced::Color = __ice_palette.colors[35]; ::ui_lang_guest::wire::Rgba([__color.r, __color.g, __color.b, __color.a]) })).map(::ui_lang_guest::wire::Background::Color), border: ::std::option::Option::Some(::ui_lang_guest::wire::Border { color: ::std::option::Option::None, width: ::std::option::Option::None, radius: ::std::option::Option::Some([(((22.0 / 2.0)) as f32).max(0.0).min(f32::MAX), (((22.0 / 2.0)) as f32).max(0.0).min(f32::MAX), (((22.0 / 2.0)) as f32).max(0.0).min(f32::MAX), (((22.0 / 2.0)) as f32).max(0.0).min(f32::MAX)]) }), snap: ::std::option::Option::None, content: ::std::boxed::Box::new({
#[cfg(test)]
let __ice_render_source_location = ::ui_lang_runtime::testing::Location::new("src/ui/kit.ice", 232, 1, "rendered view node");
#[cfg(test)]
let __ice_render_source = ::ui_lang_runtime::testing::push_render_source(__ice_render_source_location);
let __ice_rendered: __IceElement<'_, __PagesViewMessage> = ::ui_lang_guest::wire::Node::Text { options: ::ui_lang_guest::wire::TextOptions { height: ::std::option::Option::None, align_y: ::std::option::Option::None, line_height: ::std::option::Option::None, shaping: ::std::option::Option::None, wrapping: ::std::option::Option::Some(::ui_lang_guest::wire::Wrapping::None), tracking: 0.0f32, font: ::std::option::Option::Some(::ui_lang_guest::wire::NamedFont { family: ::ui_lang_guest::wire::FontFamily::Named("Geist".into()), weight: ::ui_lang_guest::wire::Weight::Semibold, stretch: ::ui_lang_guest::wire::FontStretch::Normal, style: ::ui_lang_guest::wire::FontStyle::Normal }) }, key: format!("{}/@text:232", __ice_use_scope), size: ::std::option::Option::Some(((9.0) as f32).max(f32::EPSILON).min(f32::MAX)), color: ::std::option::Option::Some({ let __color: ::iced::Color = __ice_palette.colors[5]; ::ui_lang_guest::wire::Rgba([__color.r, __color.g, __color.b, __color.a]) }), font: ::ui_lang_guest::wire::Font { monospace: false, weight: ::ui_lang_guest::wire::Weight::Normal }, width: ::std::option::Option::None, align_x: ::std::option::Option::None, content: (__ice_arg_0.to_owned()).to_string() };
#[cfg(test)]
drop(__ice_render_source);
__ice_rendered
}) } };
#[cfg(test)]
drop(__ice_render_source);
__ice_rendered
}; __component_content }
pub(super) fn __ice_component_use_14(&self, __ice_palette: __IcePalette, __ice_use_scope: ::std::string::String, __ice_cb_0: impl Fn() -> __PagesViewMessage + Clone + 'static, __ice_cb_1: impl Fn() -> __PagesViewMessage + Clone + 'static) -> __IceElement<'_, __PagesViewMessage> { let __component_content: __IceElement<'_, __PagesViewMessage> = {
#[cfg(test)]
let __ice_render_source_location = ::ui_lang_runtime::testing::Location::new("src/ui/kit.ice", 190, 1, "rendered view node");
#[cfg(test)]
let __ice_render_source = ::ui_lang_runtime::testing::push_render_source(__ice_render_source_location);
let __ice_rendered: __IceElement<'_, __PagesViewMessage> = { let __ice_node_scope = format!("{}/root", __ice_use_scope); ::ui_lang_guest::wire::Node::Container { shadow: ::ui_lang_guest::wire::Shadow { color: ::std::option::Option::Some({ let __color: ::iced::Color = __ice_palette.colors[48]; ::ui_lang_guest::wire::Rgba([__color.r, __color.g, __color.b, __color.a]) }), x: ::std::option::Option::None, y: ::std::option::Option::Some((24.0) as f32), blur: ::std::option::Option::Some((60.0) as f32) }, max_width: ::std::option::Option::None, max_height: ::std::option::Option::None, clip: false, key: __ice_node_scope.clone(), width: ::std::option::Option::Some(::ui_lang_guest::wire::Length::Fixed((418.0) as f32)), height: ::std::option::Option::None, padding: ::std::option::Option::None, align_x: ::std::option::Option::None, align_y: ::std::option::Option::None, background: (::std::option::Option::Some({ let __color: ::iced::Color = __ice_palette.colors[3]; ::ui_lang_guest::wire::Rgba([__color.r, __color.g, __color.b, __color.a]) })).map(::ui_lang_guest::wire::Background::Color), border: ::std::option::Option::Some(::ui_lang_guest::wire::Border { color: ::std::option::Option::Some({ let __color: ::iced::Color = __ice_palette.colors[39]; ::ui_lang_guest::wire::Rgba([__color.r, __color.g, __color.b, __color.a]) }), width: ::std::option::Option::Some(((1.0) as f32).max(0.0).min(f32::MAX)), radius: ::std::option::Option::Some([((14.0) as f32).max(0.0).min(f32::MAX), ((14.0) as f32).max(0.0).min(f32::MAX), ((14.0) as f32).max(0.0).min(f32::MAX), ((14.0) as f32).max(0.0).min(f32::MAX)]) }), snap: ::std::option::Option::None, content: ::std::boxed::Box::new({
#[cfg(test)]
let __ice_render_source_location = ::ui_lang_runtime::testing::Location::new("src/ui/kit.ice", 200, 1, "rendered view node");
#[cfg(test)]
let __ice_render_source = ::ui_lang_runtime::testing::push_render_source(__ice_render_source_location);
let __ice_rendered: __IceElement<'_, __PagesViewMessage> = { let mut __children: ::std::vec::Vec<__IceElement<'_, __PagesViewMessage>> = ::std::vec::Vec::new(); __children.push({
#[cfg(test)]
let __ice_render_source_location = ::ui_lang_runtime::testing::Location::new("src/ui/kit.ice", 208, 1, "rendered view node");
#[cfg(test)]
let __ice_render_source = ::ui_lang_runtime::testing::push_render_source(__ice_render_source_location);
let __ice_rendered: __IceElement<'_, __PagesViewMessage> = { let mut __children: ::std::vec::Vec<__IceElement<'_, __PagesViewMessage>> = ::std::vec::Vec::new(); __children.push({
#[cfg(test)]
let __ice_render_source_location = ::ui_lang_runtime::testing::Location::new("src/ui/kit.ice", 213, 1, "rendered view node");
#[cfg(test)]
let __ice_render_source = ::ui_lang_runtime::testing::push_render_source(__ice_render_source_location);
let __ice_rendered: __IceElement<'_, __PagesViewMessage> = ::ui_lang_guest::wire::Node::Text { options: ::ui_lang_guest::wire::TextOptions { height: ::std::option::Option::None, align_y: ::std::option::Option::None, line_height: ::std::option::Option::None, shaping: ::std::option::Option::None, wrapping: ::std::option::Option::Some(::ui_lang_guest::wire::Wrapping::None), tracking: 0.0f32, font: ::std::option::Option::Some(::ui_lang_guest::wire::NamedFont { family: ::ui_lang_guest::wire::FontFamily::Named("Geist".into()), weight: ::ui_lang_guest::wire::Weight::Semibold, stretch: ::ui_lang_guest::wire::FontStretch::Normal, style: ::ui_lang_guest::wire::FontStyle::Normal }) }, key: format!("{}/@text:213", __ice_use_scope), size: ::std::option::Option::Some(((16.0) as f32).max(f32::EPSILON).min(f32::MAX)), color: ::std::option::Option::Some({ let __color: ::iced::Color = __ice_palette.colors[7]; ::ui_lang_guest::wire::Rgba([__color.r, __color.g, __color.b, __color.a]) }), font: ::ui_lang_guest::wire::Font { monospace: false, weight: ::ui_lang_guest::wire::Weight::Normal }, width: ::std::option::Option::Some(::ui_lang_guest::wire::Length::Fill), align_x: ::std::option::Option::None, content: ("Delete this page".to_owned()).to_string() };
#[cfg(test)]
drop(__ice_render_source);
__ice_rendered
}); __children.push({
#[cfg(test)]
let __ice_render_source_location = ::ui_lang_runtime::testing::Location::new("src/ui/kit.ice", 220, 1, "rendered view node");
#[cfg(test)]
let __ice_render_source = ::ui_lang_runtime::testing::push_render_source(__ice_render_source_location);
let __ice_rendered: __IceElement<'_, __PagesViewMessage> = (|| { let __slot_content: __IceElement<'_, __PagesViewMessage> = {
#[cfg(test)]
let __ice_render_source_location = ::ui_lang_runtime::testing::Location::new("src/ui/kit.ice", 135, 1, "rendered view node");
#[cfg(test)]
let __ice_render_source = ::ui_lang_runtime::testing::push_render_source(__ice_render_source_location);
let __ice_rendered: __IceElement<'_, __PagesViewMessage> = ::ui_lang_guest::wire::Node::Button { checked: ::std::option::Option::None, expanded: ::std::option::Option::None, description: ::std::option::Option::None, key: format!("{}/@button:135", __ice_use_scope), content: ::ui_lang_guest::wire::ButtonContent::Child(::std::boxed::Box::new({
#[cfg(test)]
let __ice_render_source_location = ::ui_lang_runtime::testing::Location::new("src/ui/kit.ice", 143, 1, "rendered view node");
#[cfg(test)]
let __ice_render_source = ::ui_lang_runtime::testing::push_render_source(__ice_render_source_location);
let __ice_rendered: __IceElement<'_, __PagesViewMessage> = ::ui_lang_guest::wire::Node::Container { shadow: ::ui_lang_guest::wire::Shadow { color: ::std::option::Option::None, x: ::std::option::Option::None, y: ::std::option::Option::None, blur: ::std::option::Option::None }, max_width: ::std::option::Option::None, max_height: ::std::option::Option::None, clip: false, key: format!("{}/@container:143", __ice_use_scope), width: ::std::option::Option::Some(::ui_lang_guest::wire::Length::Fill), height: ::std::option::Option::Some(::ui_lang_guest::wire::Length::Fill), padding: ::std::option::Option::None, align_x: ::std::option::Option::Some(::ui_lang_guest::wire::AlignX::Center), align_y: ::std::option::Option::Some(::ui_lang_guest::wire::AlignY::Center), background: (::std::option::Option::None).map(::ui_lang_guest::wire::Background::Color), border: ::std::option::Option::None, snap: ::std::option::Option::None, content: ::std::boxed::Box::new({
#[cfg(test)]
let __ice_render_source_location = ::ui_lang_runtime::testing::Location::new("src/ui/kit.ice", 149, 1, "rendered view node");
#[cfg(test)]
let __ice_render_source = ::ui_lang_runtime::testing::push_render_source(__ice_render_source_location);
let __ice_rendered: __IceElement<'_, __PagesViewMessage> = ::ui_lang_guest::wire::Node::Text { options: ::ui_lang_guest::wire::TextOptions { height: ::std::option::Option::None, align_y: ::std::option::Option::None, line_height: ::std::option::Option::None, shaping: ::std::option::Option::None, wrapping: ::std::option::Option::Some(::ui_lang_guest::wire::Wrapping::None), tracking: 0.0f32, font: ::std::option::Option::Some(::ui_lang_guest::wire::NamedFont { family: ::ui_lang_guest::wire::FontFamily::Named("Geist".into()), weight: ::ui_lang_guest::wire::Weight::Normal, stretch: ::ui_lang_guest::wire::FontStretch::Normal, style: ::ui_lang_guest::wire::FontStyle::Normal }) }, key: format!("{}/@text:149", __ice_use_scope), size: ::std::option::Option::Some(((14.0) as f32).max(f32::EPSILON).min(f32::MAX)), color: ::std::option::Option::Some({ let __color: ::iced::Color = __ice_palette.colors[5]; ::ui_lang_guest::wire::Rgba([__color.r, __color.g, __color.b, __color.a]) }), font: ::ui_lang_guest::wire::Font { monospace: false, weight: ::ui_lang_guest::wire::Weight::Normal }, width: ::std::option::Option::None, align_x: ::std::option::Option::None, content: ("×".to_owned()).to_string() };
#[cfg(test)]
drop(__ice_render_source);
__ice_rendered
}) };
#[cfg(test)]
drop(__ice_render_source);
__ice_rendered
})), label: ::std::option::Option::Some(::std::string::String::from("Cancel".to_owned())), on_press: if ((self.busy || (!(self.host_error).is_empty()))) { ::std::option::Option::None } else { ::std::option::Option::Some(::ui_lang_guest::slots::message((__ice_cb_0)())) }, width: ::std::option::Option::Some(::ui_lang_guest::wire::Length::Fixed((26.0) as f32)), height: ::std::option::Option::Some(::ui_lang_guest::wire::Length::Fixed((26.0) as f32)), padding: ::std::option::Option::Some(::ui_lang_guest::wire::Edges::all((0.0) as f32)), style: ::ui_lang_guest::wire::ButtonStyle { preset: ::ui_lang_guest::wire::ButtonPreset::Primary, recipe: Some(::ui_lang_guest::wire::ButtonRecipe { base: ::ui_lang_guest::wire::Face { background: ::std::option::Option::Some({ let __color: ::iced::Color = ::iced::Color::from_rgba8(0, 0, 0, 0.000000); ::ui_lang_guest::wire::Rgba([__color.r, __color.g, __color.b, __color.a]) }), text: ::std::option::Option::Some({ let __color: ::iced::Color = __ice_palette.colors[4]; ::ui_lang_guest::wire::Rgba([__color.r, __color.g, __color.b, __color.a]) }), border: ::std::option::Option::Some(::ui_lang_guest::wire::Border { color: ::std::option::Option::None, width: ::std::option::Option::None, radius: ::std::option::Option::Some([7.0; 4]) }) }, hover_background: ::std::option::Option::Some({ let __color: ::iced::Color = __ice_palette.colors[14]; ::ui_lang_guest::wire::Rgba([__color.r, __color.g, __color.b, __color.a]) }), pressed_background: ::std::option::Option::Some({ let __color: ::iced::Color = __ice_palette.colors[39]; ::ui_lang_guest::wire::Rgba([__color.r, __color.g, __color.b, __color.a]) }), disabled_background: ::std::option::Option::None, disabled_text: ::std::option::Option::None, disabled_opacity: ::std::option::Option::Some(0.5f32), focus_ring: ::std::option::Option::Some({ let __color: ::iced::Color = __ice_palette.colors[42]; ::ui_lang_guest::wire::Rgba([__color.r, __color.g, __color.b, __color.a]) }), text_size: ::std::option::Option::Some(13.5f32), line_height: ::std::option::Option::None, font: Some(::ui_lang_guest::wire::NamedFont { family: ::ui_lang_guest::wire::FontFamily::Named("Geist".into()), weight: ::ui_lang_guest::wire::Weight::Normal, stretch: ::ui_lang_guest::wire::FontStretch::Normal, style: ::ui_lang_guest::wire::FontStyle::Normal }) }), active: ::ui_lang_guest::wire::Face { background: ::std::option::Option::Some({ let __color: ::iced::Color = ::iced::Color::from_rgba8(0, 0, 0, 0.000000); ::ui_lang_guest::wire::Rgba([__color.r, __color.g, __color.b, __color.a]) }), text: ::std::option::Option::Some({ let __color: ::iced::Color = __ice_palette.colors[5]; ::ui_lang_guest::wire::Rgba([__color.r, __color.g, __color.b, __color.a]) }), border: ::std::option::Option::Some(::ui_lang_guest::wire::Border { color: ::std::option::Option::Some({ let __color: ::iced::Color = ::iced::Color::from_rgba8(0, 0, 0, 0.000000); ::ui_lang_guest::wire::Rgba([__color.r, __color.g, __color.b, __color.a]) }), width: ::std::option::Option::Some(((1.0) as f32).max(0.0).min(f32::MAX)), radius: ::std::option::Option::Some([((7.0) as f32).max(0.0).min(f32::MAX), ((7.0) as f32).max(0.0).min(f32::MAX), ((7.0) as f32).max(0.0).min(f32::MAX), ((7.0) as f32).max(0.0).min(f32::MAX)]) }) }, hovered: ::std::option::Option::Some(::ui_lang_guest::wire::Face { background: ::std::option::Option::Some({ let __color: ::iced::Color = __ice_palette.colors[55]; ::ui_lang_guest::wire::Rgba([__color.r, __color.g, __color.b, __color.a]) }), text: ::std::option::Option::Some({ let __color: ::iced::Color = __ice_palette.colors[4]; ::ui_lang_guest::wire::Rgba([__color.r, __color.g, __color.b, __color.a]) }), border: ::std::option::Option::None }), pressed: ::std::option::Option::Some(::ui_lang_guest::wire::Face { background: ::std::option::Option::Some({ let __color: ::iced::Color = __ice_palette.colors[56]; ::ui_lang_guest::wire::Rgba([__color.r, __color.g, __color.b, __color.a]) }), text: ::std::option::Option::Some({ let __color: ::iced::Color = __ice_palette.colors[4]; ::ui_lang_guest::wire::Rgba([__color.r, __color.g, __color.b, __color.a]) }), border: ::std::option::Option::None }), disabled: ::std::option::Option::None } };
#[cfg(test)]
drop(__ice_render_source);
__ice_rendered
}; __slot_content })();
#[cfg(test)]
drop(__ice_render_source);
__ice_rendered
}); ::ui_lang_guest::wire::Node::Linear { max_width: ::std::option::Option::None, clip: false, key: format!("{}/@layout:208", __ice_use_scope), wrap: None, axis: ::ui_lang_guest::wire::Axis::Row, spacing: ::std::option::Option::Some((10.0) as f32), padding: ::std::option::Option::None, width: ::std::option::Option::Some(::ui_lang_guest::wire::Length::Fill), height: ::std::option::Option::None, align: ::std::option::Option::Some(::ui_lang_guest::wire::AlignX::Center), background: ::std::option::Option::None, border: ::std::option::Option::None, children: __children } };
#[cfg(test)]
drop(__ice_render_source);
__ice_rendered
}); __children.push({
#[cfg(test)]
let __ice_render_source_location = ::ui_lang_runtime::testing::Location::new("src/ui/kit.ice", 221, 1, "rendered view node");
#[cfg(test)]
let __ice_render_source = ::ui_lang_runtime::testing::push_render_source(__ice_render_source_location);
let __ice_rendered: __IceElement<'_, __PagesViewMessage> = (|| { let __slot_content: __IceElement<'_, __PagesViewMessage> = {
#[cfg(test)]
let __ice_render_source_location = ::ui_lang_runtime::testing::Location::new("src/ui/kit.ice", 158, 1, "rendered view node");
#[cfg(test)]
let __ice_render_source = ::ui_lang_runtime::testing::push_render_source(__ice_render_source_location);
let __ice_rendered: __IceElement<'_, __PagesViewMessage> = { let mut __children: ::std::vec::Vec<__IceElement<'_, __PagesViewMessage>> = ::std::vec::Vec::new(); __children.push({
#[cfg(test)]
let __ice_render_source_location = ::ui_lang_runtime::testing::Location::new("src/ui/kit.ice", 159, 1, "rendered view node");
#[cfg(test)]
let __ice_render_source = ::ui_lang_runtime::testing::push_render_source(__ice_render_source_location);
let __ice_rendered: __IceElement<'_, __PagesViewMessage> = ::ui_lang_guest::wire::Node::Text { options: ::ui_lang_guest::wire::TextOptions { height: ::std::option::Option::None, align_y: ::std::option::Option::None, line_height: ::std::option::Option::None, shaping: ::std::option::Option::None, wrapping: ::std::option::Option::None, tracking: 0.0f32, font: ::std::option::Option::Some(::ui_lang_guest::wire::NamedFont { family: ::ui_lang_guest::wire::FontFamily::Named("Geist".into()), weight: ::ui_lang_guest::wire::Weight::Medium, stretch: ::ui_lang_guest::wire::FontStretch::Normal, style: ::ui_lang_guest::wire::FontStyle::Normal }) }, key: format!("{}/@text:159", __ice_use_scope), size: ::std::option::Option::Some(((13.5) as f32).max(f32::EPSILON).min(f32::MAX)), color: ::std::option::Option::Some({ let __color: ::iced::Color = __ice_palette.colors[4]; ::ui_lang_guest::wire::Rgba([__color.r, __color.g, __color.b, __color.a]) }), font: ::ui_lang_guest::wire::Font { monospace: false, weight: ::ui_lang_guest::wire::Weight::Normal }, width: ::std::option::Option::Some(::ui_lang_guest::wire::Length::Fill), align_x: ::std::option::Option::None, content: (crate::host::keep_str((!(self.active_page_title).is_empty()), ::std::convert::AsRef::as_ref(&(self.active_page_title)), ::std::convert::AsRef::as_ref(&("Untitled"))).to_owned()).to_string() };
#[cfg(test)]
drop(__ice_render_source);
__ice_rendered
}); __children.push({
#[cfg(test)]
let __ice_render_source_location = ::ui_lang_runtime::testing::Location::new("src/ui/kit.ice", 165, 1, "rendered view node");
#[cfg(test)]
let __ice_render_source = ::ui_lang_runtime::testing::push_render_source(__ice_render_source_location);
let __ice_rendered: __IceElement<'_, __PagesViewMessage> = ::ui_lang_guest::wire::Node::Text { options: ::ui_lang_guest::wire::TextOptions { height: ::std::option::Option::None, align_y: ::std::option::Option::None, line_height: ::std::option::Option::Some(::ui_lang_guest::wire::LineHeight::Relative(((1.5) as f32).max(f32::EPSILON).min(f32::MAX))), shaping: ::std::option::Option::None, wrapping: ::std::option::Option::None, tracking: 0.0f32, font: ::std::option::Option::Some(::ui_lang_guest::wire::NamedFont { family: ::ui_lang_guest::wire::FontFamily::Named("Geist".into()), weight: ::ui_lang_guest::wire::Weight::Normal, stretch: ::ui_lang_guest::wire::FontStretch::Normal, style: ::ui_lang_guest::wire::FontStyle::Normal }) }, key: format!("{}/@text:165", __ice_use_scope), size: ::std::option::Option::Some(((12.0) as f32).max(f32::EPSILON).min(f32::MAX)), color: ::std::option::Option::Some({ let __color: ::iced::Color = __ice_palette.colors[70]; ::ui_lang_guest::wire::Rgba([__color.r, __color.g, __color.b, __color.a]) }), font: ::ui_lang_guest::wire::Font { monospace: false, weight: ::ui_lang_guest::wire::Weight::Normal }, width: ::std::option::Option::Some(::ui_lang_guest::wire::Length::Fill), align_x: ::std::option::Option::None, content: ("Everything nested under it goes too — its blocks, any subpages beneath them, and every comment thread on any of it — for every member. This cannot be undone from the app.".to_owned()).to_string() };
#[cfg(test)]
drop(__ice_render_source);
__ice_rendered
}); __children.push({
#[cfg(test)]
let __ice_render_source_location = ::ui_lang_runtime::testing::Location::new("src/ui/kit.ice", 171, 1, "rendered view node");
#[cfg(test)]
let __ice_render_source = ::ui_lang_runtime::testing::push_render_source(__ice_render_source_location);
let __ice_rendered: __IceElement<'_, __PagesViewMessage> = { let mut __children: ::std::vec::Vec<__IceElement<'_, __PagesViewMessage>> = ::std::vec::Vec::new(); __children.push({
#[cfg(test)]
let __ice_render_source_location = ::ui_lang_runtime::testing::Location::new("src/ui/kit.ice", 176, 1, "rendered view node");
#[cfg(test)]
let __ice_render_source = ::ui_lang_runtime::testing::push_render_source(__ice_render_source_location);
let __ice_rendered: __IceElement<'_, __PagesViewMessage> = ::ui_lang_guest::wire::Node::Button { checked: ::std::option::Option::None, expanded: ::std::option::Option::None, description: ::std::option::Option::None, key: format!("{}/@button:176", __ice_use_scope), content: ::ui_lang_guest::wire::ButtonContent::Label(::std::string::String::from("Cancel")), label: ::std::option::Option::None, on_press: if ((self.busy || (!(self.host_error).is_empty()))) { ::std::option::Option::None } else { ::std::option::Option::Some(::ui_lang_guest::slots::message((__ice_cb_0)())) }, width: ::std::option::Option::None, height: ::std::option::Option::None, padding: ::std::option::Option::Some(::ui_lang_guest::wire::Edges::all((7.0) as f32)), style: ::ui_lang_guest::wire::ButtonStyle { preset: ::ui_lang_guest::wire::ButtonPreset::Primary, recipe: Some(::ui_lang_guest::wire::ButtonRecipe { base: ::ui_lang_guest::wire::Face { background: ::std::option::Option::Some({ let __color: ::iced::Color = __ice_palette.colors[12]; ::ui_lang_guest::wire::Rgba([__color.r, __color.g, __color.b, __color.a]) }), text: ::std::option::Option::Some({ let __color: ::iced::Color = __ice_palette.colors[13]; ::ui_lang_guest::wire::Rgba([__color.r, __color.g, __color.b, __color.a]) }), border: ::std::option::Option::Some(::ui_lang_guest::wire::Border { color: ::std::option::Option::Some({ let __color: ::iced::Color = __ice_palette.colors[40]; ::ui_lang_guest::wire::Rgba([__color.r, __color.g, __color.b, __color.a]) }), width: ::std::option::Option::Some(1.0), radius: ::std::option::Option::Some([9.0; 4]) }) }, hover_background: ::std::option::Option::Some({ let __color: ::iced::Color = __ice_palette.colors[14]; ::ui_lang_guest::wire::Rgba([__color.r, __color.g, __color.b, __color.a]) }), pressed_background: ::std::option::Option::Some({ let __color: ::iced::Color = __ice_palette.colors[6]; ::ui_lang_guest::wire::Rgba([__color.r, __color.g, __color.b, __color.a]) }), disabled_background: ::std::option::Option::None, disabled_text: ::std::option::Option::None, disabled_opacity: ::std::option::Option::Some(0.5f32), focus_ring: ::std::option::Option::Some({ let __color: ::iced::Color = __ice_palette.colors[42]; ::ui_lang_guest::wire::Rgba([__color.r, __color.g, __color.b, __color.a]) }), text_size: ::std::option::Option::Some(12.5f32), line_height: ::std::option::Option::None, font: Some(::ui_lang_guest::wire::NamedFont { family: ::ui_lang_guest::wire::FontFamily::Named("Geist".into()), weight: ::ui_lang_guest::wire::Weight::Semibold, stretch: ::ui_lang_guest::wire::FontStretch::Normal, style: ::ui_lang_guest::wire::FontStyle::Normal }) }), active: ::ui_lang_guest::wire::Face::default(), hovered: ::std::option::Option::None, pressed: ::std::option::Option::None, disabled: ::std::option::Option::None } };
#[cfg(test)]
drop(__ice_render_source);
__ice_rendered
}); __children.push({
#[cfg(test)]
let __ice_render_source_location = ::ui_lang_runtime::testing::Location::new("src/ui/kit.ice", 181, 1, "rendered view node");
#[cfg(test)]
let __ice_render_source = ::ui_lang_runtime::testing::push_render_source(__ice_render_source_location);
let __ice_rendered: __IceElement<'_, __PagesViewMessage> = ::ui_lang_guest::wire::Node::Button { checked: ::std::option::Option::None, expanded: ::std::option::Option::None, description: ::std::option::Option::None, key: format!("{}/@button:181", __ice_use_scope), content: ::ui_lang_guest::wire::ButtonContent::Child(::std::boxed::Box::new({
#[cfg(test)]
let __ice_render_source_location = ::ui_lang_runtime::testing::Location::new("src/ui/kit.ice", 187, 1, "rendered view node");
#[cfg(test)]
let __ice_render_source = ::ui_lang_runtime::testing::push_render_source(__ice_render_source_location);
let __ice_rendered: __IceElement<'_, __PagesViewMessage> = ::ui_lang_guest::wire::Node::Text { options: ::ui_lang_guest::wire::TextOptions { height: ::std::option::Option::None, align_y: ::std::option::Option::None, line_height: ::std::option::Option::None, shaping: ::std::option::Option::None, wrapping: ::std::option::Option::Some(::ui_lang_guest::wire::Wrapping::None), tracking: 0.0f32, font: ::std::option::Option::Some(::ui_lang_guest::wire::NamedFont { family: ::ui_lang_guest::wire::FontFamily::Named("Geist".into()), weight: ::ui_lang_guest::wire::Weight::Normal, stretch: ::ui_lang_guest::wire::FontStretch::Normal, style: ::ui_lang_guest::wire::FontStyle::Normal }) }, key: format!("{}/@text:187", __ice_use_scope), size: ::std::option::Option::Some(((13.0) as f32).max(f32::EPSILON).min(f32::MAX)), color: ::std::option::Option::None, font: ::ui_lang_guest::wire::Font { monospace: false, weight: ::ui_lang_guest::wire::Weight::Normal }, width: ::std::option::Option::None, align_x: ::std::option::Option::None, content: ("Delete page".to_owned()).to_string() };
#[cfg(test)]
drop(__ice_render_source);
__ice_rendered
})), label: ::std::option::Option::Some(::std::string::String::from("Delete page".to_owned())), on_press: if ((self.busy || (!(self.host_error).is_empty()))) { ::std::option::Option::None } else { ::std::option::Option::Some(::ui_lang_guest::slots::message((__ice_cb_1)())) }, width: ::std::option::Option::None, height: ::std::option::Option::None, padding: ::std::option::Option::Some(::ui_lang_guest::wire::Edges::all((7.0) as f32)), style: ::ui_lang_guest::wire::ButtonStyle { preset: ::ui_lang_guest::wire::ButtonPreset::Primary, recipe: Some(::ui_lang_guest::wire::ButtonRecipe { base: ::ui_lang_guest::wire::Face { background: ::std::option::Option::Some({ let __color: ::iced::Color = __ice_palette.colors[20]; ::ui_lang_guest::wire::Rgba([__color.r, __color.g, __color.b, __color.a]) }), text: ::std::option::Option::Some({ let __color: ::iced::Color = __ice_palette.colors[21]; ::ui_lang_guest::wire::Rgba([__color.r, __color.g, __color.b, __color.a]) }), border: ::std::option::Option::Some(::ui_lang_guest::wire::Border { color: ::std::option::Option::None, width: ::std::option::Option::None, radius: ::std::option::Option::Some([9.0; 4]) }) }, hover_background: ::std::option::Option::Some({ let __color: ::iced::Color = { let mut __color = __ice_palette.colors[20]; __color.a = 0.900000; __color }; ::ui_lang_guest::wire::Rgba([__color.r, __color.g, __color.b, __color.a]) }), pressed_background: ::std::option::Option::Some({ let __color: ::iced::Color = { let mut __color = __ice_palette.colors[20]; __color.a = 0.800000; __color }; ::ui_lang_guest::wire::Rgba([__color.r, __color.g, __color.b, __color.a]) }), disabled_background: ::std::option::Option::None, disabled_text: ::std::option::Option::None, disabled_opacity: ::std::option::Option::Some(0.5f32), focus_ring: ::std::option::Option::Some({ let __color: ::iced::Color = __ice_palette.colors[42]; ::ui_lang_guest::wire::Rgba([__color.r, __color.g, __color.b, __color.a]) }), text_size: ::std::option::Option::Some(12.5f32), line_height: ::std::option::Option::None, font: Some(::ui_lang_guest::wire::NamedFont { family: ::ui_lang_guest::wire::FontFamily::Named("Geist".into()), weight: ::ui_lang_guest::wire::Weight::Semibold, stretch: ::ui_lang_guest::wire::FontStretch::Normal, style: ::ui_lang_guest::wire::FontStyle::Normal }) }), active: ::ui_lang_guest::wire::Face::default(), hovered: ::std::option::Option::None, pressed: ::std::option::Option::None, disabled: ::std::option::Option::None } };
#[cfg(test)]
drop(__ice_render_source);
__ice_rendered
}); ::ui_lang_guest::wire::Node::Linear { max_width: ::std::option::Option::None, clip: false, key: format!("{}/@layout:171", __ice_use_scope), wrap: None, axis: ::ui_lang_guest::wire::Axis::Row, spacing: ::std::option::Option::Some((8.0) as f32), padding: ::std::option::Option::None, width: ::std::option::Option::Some(::ui_lang_guest::wire::Length::Fill), height: ::std::option::Option::None, align: ::std::option::Option::Some(::ui_lang_guest::wire::AlignX::Right), background: ::std::option::Option::None, border: ::std::option::Option::None, children: __children } };
#[cfg(test)]
drop(__ice_render_source);
__ice_rendered
}); ::ui_lang_guest::wire::Node::Linear { max_width: ::std::option::Option::None, clip: false, key: format!("{}/@layout:158", __ice_use_scope), wrap: None, axis: ::ui_lang_guest::wire::Axis::Column, spacing: ::std::option::Option::Some((13.0) as f32), padding: ::std::option::Option::None, width: ::std::option::Option::Some(::ui_lang_guest::wire::Length::Fill), height: ::std::option::Option::None, align: ::std::option::Option::None, background: ::std::option::Option::None, border: ::std::option::Option::None, children: __children } };
#[cfg(test)]
drop(__ice_render_source);
__ice_rendered
}; __slot_content })();
#[cfg(test)]
drop(__ice_render_source);
__ice_rendered
}); ::ui_lang_guest::wire::Node::Linear { max_width: ::std::option::Option::None, clip: false, key: format!("{}/@layout:200", __ice_use_scope), wrap: None, axis: ::ui_lang_guest::wire::Axis::Column, spacing: ::std::option::Option::Some((13.0) as f32), padding: ::std::option::Option::Some(::ui_lang_guest::wire::Edges { top: (20.0) as f32, right: (22.0) as f32, bottom: (22.0) as f32, left: (22.0) as f32 }), width: ::std::option::Option::Some(::ui_lang_guest::wire::Length::Fill), height: ::std::option::Option::None, align: ::std::option::Option::None, background: ::std::option::Option::None, border: ::std::option::Option::None, children: __children } };
#[cfg(test)]
drop(__ice_render_source);
__ice_rendered
}) } };
#[cfg(test)]
drop(__ice_render_source);
__ice_rendered
}; __component_content }
pub(super) fn __ice_component_use_15(&self, __ice_palette: __IcePalette, __ice_use_scope: ::std::string::String, __ice_cb_0: impl Fn() -> __PagesViewMessage + Clone + 'static, __ice_cb_1: impl Fn(::std::string::String) -> __PagesViewMessage + Clone + 'static, __ice_cb_2: impl Fn() -> __PagesViewMessage + Clone + 'static, __ice_cb_3: impl Fn() -> __PagesViewMessage + Clone + 'static, __ice_cb_4: impl Fn() -> __PagesViewMessage + Clone + 'static, __ice_cb_5: impl Fn(::std::string::String, ::std::string::String) -> __PagesViewMessage + Clone + 'static, __ice_cb_6: impl Fn() -> __PagesViewMessage + Clone + 'static, __ice_cb_7: impl Fn() -> __PagesViewMessage + Clone + 'static, __ice_cb_8: impl Fn() -> __PagesViewMessage + Clone + 'static, __ice_cb_9: impl Fn(::std::string::String) -> __PagesViewMessage + Clone + 'static, __ice_cb_10: impl Fn(f64, f64) -> __PagesViewMessage + Clone + 'static, __ice_cb_11: impl Fn(::std::string::String) -> __PagesViewMessage + Clone + 'static, __ice_cb_12: impl Fn(::std::string::String, ::std::string::String) -> __PagesViewMessage + Clone + 'static, __ice_cb_13: impl Fn() -> __PagesViewMessage + Clone + 'static, __ice_cb_14: impl Fn(::std::string::String) -> __PagesViewMessage + Clone + 'static, __ice_cb_15: impl Fn(f64, f64) -> __PagesViewMessage + Clone + 'static, __ice_cb_16: impl Fn(f64, f64) -> __PagesViewMessage + Clone + 'static, __ice_cb_17: impl Fn(::std::string::String, bool) -> __PagesViewMessage + Clone + 'static, __ice_cb_18: impl Fn() -> __PagesViewMessage + Clone + 'static, __ice_cb_19: impl Fn(::std::string::String) -> __PagesViewMessage + Clone + 'static, __ice_cb_20: impl Fn() -> __PagesViewMessage + Clone + 'static, __ice_cb_21: impl Fn() -> __PagesViewMessage + Clone + 'static, __ice_cb_22: impl Fn() -> __PagesViewMessage + Clone + 'static, __ice_cb_23: impl Fn() -> __PagesViewMessage + Clone + 'static, __ice_cb_24: impl Fn(::std::string::String) -> __PagesViewMessage + Clone + 'static, __ice_cb_25: impl Fn(::std::string::String) -> __PagesViewMessage + Clone + 'static, __ice_cb_26: impl Fn() -> __PagesViewMessage + Clone + 'static) -> __IceElement<'_, __PagesViewMessage> { let __component_content: __IceElement<'_, __PagesViewMessage> = {
#[cfg(test)]
let __ice_render_source_location = ::ui_lang_runtime::testing::Location::new("src/ui/kit.ice", 133, 1, "rendered view node");
#[cfg(test)]
let __ice_render_source = ::ui_lang_runtime::testing::push_render_source(__ice_render_source_location);
let __ice_rendered: __IceElement<'_, __PagesViewMessage> = ::ui_lang_runtime::grow_stack(|| self.__ice_component_use_14(__ice_palette, format!("{}/ModalShell@2060", __ice_use_scope), ({ let __route_callback = (__ice_cb_8).clone(); move || (__route_callback)() }).clone(), ({ let __route_callback = (__ice_cb_7).clone(); move || (__route_callback)() }).clone()));
#[cfg(test)]
drop(__ice_render_source);
__ice_rendered
}; __component_content }
}
}
