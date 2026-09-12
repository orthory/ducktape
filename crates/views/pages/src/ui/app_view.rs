#[allow(warnings, clippy::all)]
mod __ice_group_app_view {
use super::*;
impl super::PagesView {
pub(super) fn __view(&self) -> __IceElement<'_, __PagesViewMessage> { let __ice_view = ::ui_lang_runtime::dev::Span::view("PagesView", "src/ui/app.ice:721"); let __ice_palette = self.__palette(); let __ice_app_theme = Self::__app_theme(__ice_palette); let __ice_content: __IceElement<'_, __PagesViewMessage> = {
#[cfg(test)]
let __ice_render_source_location = ::ui_lang_runtime::testing::Location::new("src/ui/app.ice", 721, 1, "rendered view node");
#[cfg(test)]
let __ice_render_source = ::ui_lang_runtime::testing::push_render_source(__ice_render_source_location);
let __ice_rendered: __IceElement<'_, __PagesViewMessage> = ::ui_lang_guest::wire::Node::Sensor { key: format!("{}/@sensor:721", "PagesView"), reset: ::std::option::Option::None, on_show: ::std::option::Option::Some(::ui_lang_guest::slots::handler::<(f32, f32), __PagesViewMessage>(::std::boxed::Box::new({ let __route = {  move |__size: (f64, f64)| __PagesViewMessage::PagesViewportChanged(__size.0, __size.1) }; move |__sent: (f32, f32)| ::std::option::Option::Some(__route((f64::from(__sent.0), f64::from(__sent.1)))) }))), on_resize: ::std::option::Option::Some(::ui_lang_guest::slots::handler::<(f32, f32), __PagesViewMessage>(::std::boxed::Box::new({ let __route = {  move |__size: (f64, f64)| __PagesViewMessage::PagesViewportChanged(__size.0, __size.1) }; move |__sent: (f32, f32)| ::std::option::Option::Some(__route((f64::from(__sent.0), f64::from(__sent.1)))) }))), on_hide: ::std::option::Option::None, anticipate: ::std::option::Option::None, delay: ::std::option::Option::None, child: ::std::boxed::Box::new({
#[cfg(test)]
let __ice_render_source_location = ::ui_lang_runtime::testing::Location::new("src/ui/app.ice", 722, 1, "rendered view node");
#[cfg(test)]
let __ice_render_source = ::ui_lang_runtime::testing::push_render_source(__ice_render_source_location);
let __ice_rendered: __IceElement<'_, __PagesViewMessage> = { let __ice_node_scope = format!("{}/root", "PagesView"); ::ui_lang_guest::wire::Node::Container { shadow: ::ui_lang_guest::wire::Shadow { color: ::std::option::Option::None, x: ::std::option::Option::None, y: ::std::option::Option::None, blur: ::std::option::Option::None }, max_width: ::std::option::Option::None, max_height: ::std::option::Option::None, clip: false, key: __ice_node_scope.clone(), width: ::std::option::Option::Some(::ui_lang_guest::wire::Length::Fill), height: ::std::option::Option::Some(::ui_lang_guest::wire::Length::Fill), padding: ::std::option::Option::None, align_x: ::std::option::Option::None, align_y: ::std::option::Option::None, background: (::std::option::Option::Some({ let __color: ::iced::Color = __ice_palette.colors[2]; ::ui_lang_guest::wire::Rgba([__color.r, __color.g, __color.b, __color.a]) })).map(::ui_lang_guest::wire::Background::Color), border: ::std::option::Option::None, snap: ::std::option::Option::None, content: ::std::boxed::Box::new({
#[cfg(test)]
let __ice_render_source_location = ::ui_lang_runtime::testing::Location::new("src/ui/app.ice", 727, 1, "rendered view node");
#[cfg(test)]
let __ice_render_source = ::ui_lang_runtime::testing::push_render_source(__ice_render_source_location);
let __ice_rendered: __IceElement<'_, __PagesViewMessage> = { let __ice_node_scope = format!("{}/pages", __ice_node_scope); ::ui_lang_runtime::grow_stack(|| self.__ice_component_use_16(__ice_palette, __ice_node_scope.clone())) };
#[cfg(test)]
drop(__ice_render_source);
__ice_rendered
}) } };
#[cfg(test)]
drop(__ice_render_source);
__ice_rendered
}) };
#[cfg(test)]
drop(__ice_render_source);
__ice_rendered
}; let __ice_root: __IceElement<'_, __PagesViewMessage> = __ice_content; __ice_root }

}
}
