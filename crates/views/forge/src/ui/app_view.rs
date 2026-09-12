#[allow(warnings, clippy::all)]
mod __ice_group_app_view {
use super::*;
impl super::ForgeView {
pub(super) fn __view(&self) -> __IceElement<'_, __ForgeViewMessage> { let __ice_palette = self.__palette(); let __ice_content: __IceElement<'_, __ForgeViewMessage> = {
let __ice_rendered: __IceElement<'_, __ForgeViewMessage> = { let mut __children: ::std::vec::Vec<__IceElement<'_, __ForgeViewMessage>> = ::std::vec::Vec::new(); __children.push({
let __ice_rendered: __IceElement<'_, __ForgeViewMessage> = ::ducktape_view_guest::wire::Node::Sensor { key: format!("{}/@sensor:655", "ForgeView"), reset: ::std::option::Option::None, on_show: ::std::option::Option::Some(::ducktape_view_guest::slots::handler::<(f32, f32), __ForgeViewMessage>(::std::boxed::Box::new({ let __route = {  move |__size: (f64, f64)| __ForgeViewMessage::ViewportChanged(__size.0, __size.1) }; move |__sent: (f32, f32)| ::std::option::Option::Some(__route((f64::from(__sent.0), f64::from(__sent.1)))) }))), on_resize: ::std::option::Option::Some(::ducktape_view_guest::slots::handler::<(f32, f32), __ForgeViewMessage>(::std::boxed::Box::new({ let __route = {  move |__size: (f64, f64)| __ForgeViewMessage::ViewportChanged(__size.0, __size.1) }; move |__sent: (f32, f32)| ::std::option::Option::Some(__route((f64::from(__sent.0), f64::from(__sent.1)))) }))), on_hide: ::std::option::Option::None, anticipate: ::std::option::Option::None, delay: ::std::option::Option::None, child: ::std::boxed::Box::new({
let __ice_rendered: __IceElement<'_, __ForgeViewMessage> = ::ducktape_view_guest::wire::Node::Space { width: ::std::option::Option::Some(::ducktape_view_guest::wire::Length::Fill), height: ::std::option::Option::Some(::ducktape_view_guest::wire::Length::Fixed((0.0) as f32)) };
__ice_rendered
}) };
__ice_rendered
}); if ((!(self.host_error).is_empty()) && (self.repos).is_empty()) { __children.push({
let __ice_rendered: __IceElement<'_, __ForgeViewMessage> = ::ducktape_view_guest::wire::Node::Container { shadow: ::ducktape_view_guest::wire::Shadow { color: ::std::option::Option::None, x: ::std::option::Option::None, y: ::std::option::Option::None, blur: ::std::option::Option::None }, max_width: ::std::option::Option::None, max_height: ::std::option::Option::None, clip: false, key: format!("{}/@container:658", "ForgeView"), width: ::std::option::Option::Some(::ducktape_view_guest::wire::Length::Fill), height: ::std::option::Option::Some(::ducktape_view_guest::wire::Length::Fill), padding: ::std::option::Option::Some(::ducktape_view_guest::wire::Edges { top: (22.0) as f32, right: (22.0) as f32, bottom: (22.0) as f32, left: (22.0) as f32 }), align_x: ::std::option::Option::None, align_y: ::std::option::Option::None, background: (::std::option::Option::None).map(::ducktape_view_guest::wire::Background::Color), border: ::std::option::Option::None, snap: ::std::option::Option::None, content: ::std::boxed::Box::new({
let __ice_rendered: __IceElement<'_, __ForgeViewMessage> = (|| self.__ice_component_use_0(__ice_palette, format!("{}/EmptyState@4442", "ForgeView")))();
__ice_rendered
}) };
__ice_rendered
}); } if ((self.host_error).is_empty() || (!(self.repos).is_empty())) { __children.push({
let __ice_rendered: __IceElement<'_, __ForgeViewMessage> = { let mut __children: ::std::vec::Vec<__IceElement<'_, __ForgeViewMessage>> = ::std::vec::Vec::new(); __children.push({
let __ice_rendered: __IceElement<'_, __ForgeViewMessage> = { let __ice_node_scope = format!("{}/forge", "ForgeView"); (|| self.__ice_component_use_96(__ice_palette, __ice_node_scope.clone()))() };
__ice_rendered
}); ::ducktape_view_guest::wire::Node::Linear { max_width: ::std::option::Option::None, clip: false, key: format!("{}/@layout:666", "ForgeView"), wrap: None, axis: ::ducktape_view_guest::wire::Axis::Column, spacing: ::std::option::Option::None, padding: ::std::option::Option::None, width: ::std::option::Option::Some(::ducktape_view_guest::wire::Length::Fill), height: ::std::option::Option::Some(::ducktape_view_guest::wire::Length::Fill), align: ::std::option::Option::None, background: ::std::option::Option::None, border: ::std::option::Option::None, children: __children } };
__ice_rendered
}); } ::ducktape_view_guest::wire::Node::Linear { max_width: ::std::option::Option::None, clip: false, key: format!("{}/@layout:654", "ForgeView"), wrap: None, axis: ::ducktape_view_guest::wire::Axis::Column, spacing: ::std::option::Option::None, padding: ::std::option::Option::None, width: ::std::option::Option::Some(::ducktape_view_guest::wire::Length::Fill), height: ::std::option::Option::Some(::ducktape_view_guest::wire::Length::Fill), align: ::std::option::Option::None, background: ::std::option::Option::None, border: ::std::option::Option::None, children: __children } };
__ice_rendered
}; let __ice_root: __IceElement<'_, __ForgeViewMessage> = __ice_content; __ice_root }

}
}
