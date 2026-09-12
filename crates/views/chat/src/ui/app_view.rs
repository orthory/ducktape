#[allow(warnings, clippy::all)]
mod __ice_group_app_view {
use super::*;
impl super::ChatView {
pub(super) fn __view(&self) -> __IceElement<'_, __ChatViewMessage> { let __ice_palette = self.__palette(); let __ice_content: __IceElement<'_, __ChatViewMessage> = {
let __ice_rendered: __IceElement<'_, __ChatViewMessage> = ::ui_lang_guest::wire::Node::Sensor { key: format!("{}/@sensor:906", "ChatView"), reset: ::std::option::Option::None, on_show: ::std::option::Option::Some(::ui_lang_guest::slots::handler::<(f32, f32), __ChatViewMessage>(::std::boxed::Box::new({ let __route = {  move |__size: (f64, f64)| __ChatViewMessage::ChatViewportChanged(__size.0, __size.1) }; move |__sent: (f32, f32)| ::std::option::Option::Some(__route((f64::from(__sent.0), f64::from(__sent.1)))) }))), on_resize: ::std::option::Option::Some(::ui_lang_guest::slots::handler::<(f32, f32), __ChatViewMessage>(::std::boxed::Box::new({ let __route = {  move |__size: (f64, f64)| __ChatViewMessage::ChatViewportChanged(__size.0, __size.1) }; move |__sent: (f32, f32)| ::std::option::Option::Some(__route((f64::from(__sent.0), f64::from(__sent.1)))) }))), on_hide: ::std::option::Option::None, anticipate: ::std::option::Option::None, delay: ::std::option::Option::None, child: ::std::boxed::Box::new({
let __ice_rendered: __IceElement<'_, __ChatViewMessage> = { let __ice_node_scope = format!("{}/chat", "ChatView"); ::ui_lang_runtime::grow_stack(|| self.__ice_component_use_50(__ice_palette, __ice_node_scope.clone())) };
__ice_rendered
}) };
__ice_rendered
}; let __ice_root: __IceElement<'_, __ChatViewMessage> = __ice_content; __ice_root }

}
}
