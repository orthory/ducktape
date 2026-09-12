#[allow(warnings, clippy::all)]
mod __ice_group_pages_f7d33d26 {
use super::*;
impl super::PagesView {
pub(super) fn __ice_component_use_16(&self, __ice_palette: __IcePalette, __ice_use_scope: ::std::string::String) -> __IceElement<'_, __PagesViewMessage> { let __component_content: __IceElement<'_, __PagesViewMessage> = {
#[cfg(test)]
let __ice_render_source_location = ::ui_lang_runtime::testing::Location::new("src/ui/pages.ice", 33, 1, "rendered view node");
#[cfg(test)]
let __ice_render_source = ::ui_lang_runtime::testing::push_render_source(__ice_render_source_location);
let __ice_rendered: __IceElement<'_, __PagesViewMessage> = { let mut __children: ::std::vec::Vec<__IceElement<'_, __PagesViewMessage>> = ::std::vec::Vec::new(); __children.push({
#[cfg(test)]
let __ice_render_source_location = ::ui_lang_runtime::testing::Location::new("src/ui/pages.ice", 34, 1, "rendered view node");
#[cfg(test)]
let __ice_render_source = ::ui_lang_runtime::testing::push_render_source(__ice_render_source_location);
let __ice_rendered: __IceElement<'_, __PagesViewMessage> = { let __ice_node_scope = format!("{}/page-list", __ice_use_scope); ::ui_lang_guest::wire::Node::Container { shadow: ::ui_lang_guest::wire::Shadow { color: ::std::option::Option::None, x: ::std::option::Option::None, y: ::std::option::Option::None, blur: ::std::option::Option::None }, max_width: ::std::option::Option::None, max_height: ::std::option::Option::None, clip: true, key: __ice_node_scope.clone(), width: ::std::option::Option::Some(::ui_lang_guest::wire::Length::Fixed((self.sidebar_width) as f32)), height: ::std::option::Option::Some(::ui_lang_guest::wire::Length::Fill), padding: ::std::option::Option::None, align_x: ::std::option::Option::None, align_y: ::std::option::Option::None, background: (::std::option::Option::Some({ let __color: ::iced::Color = __ice_palette.colors[54]; ::ui_lang_guest::wire::Rgba([__color.r, __color.g, __color.b, __color.a]) })).map(::ui_lang_guest::wire::Background::Color), border: ::std::option::Option::None, snap: ::std::option::Option::None, content: ::std::boxed::Box::new({
#[cfg(test)]
let __ice_render_source_location = ::ui_lang_runtime::testing::Location::new("src/ui/pages.ice", 40, 1, "rendered view node");
#[cfg(test)]
let __ice_render_source = ::ui_lang_runtime::testing::push_render_source(__ice_render_source_location);
let __ice_rendered: __IceElement<'_, __PagesViewMessage> = { let mut __children: ::std::vec::Vec<__IceElement<'_, __PagesViewMessage>> = ::std::vec::Vec::new(); __children.push({
#[cfg(test)]
let __ice_render_source_location = ::ui_lang_runtime::testing::Location::new("src/ui/pages.ice", 49, 1, "rendered view node");
#[cfg(test)]
let __ice_render_source = ::ui_lang_runtime::testing::push_render_source(__ice_render_source_location);
let __ice_rendered: __IceElement<'_, __PagesViewMessage> = ::ui_lang_runtime::grow_stack(|| self.__ice_component_use_1(__ice_palette, format!("{}/SidebarHeader@670", __ice_use_scope), (move || __PagesViewMessage::ArmPageDelete).clone(), (move |__event_0| __PagesViewMessage::ChoosePage(__event_0)).clone(), (move || __PagesViewMessage::ClearPageSearch).clone(), (move || __PagesViewMessage::CloseBlockComments).clone(), (move || __PagesViewMessage::ClosePageMenu).clone(), (move |__event_0, __event_1| __PagesViewMessage::CopyToClipboard(__event_0, __event_1)).clone(), (move || __PagesViewMessage::CreatePageSubmit).clone(), (move || __PagesViewMessage::DeletePageSubmit).clone(), (move || __PagesViewMessage::DisarmPageDelete).clone(), (move |__event_0| __PagesViewMessage::DiscardOrphanedCommentDraft(__event_0)).clone(), (move |__event_0, __event_1| __PagesViewMessage::CommentsCardMeasured(__event_0, __event_1)).clone(), (move |__event_0| __PagesViewMessage::NarrowCommentScope(__event_0)).clone(), (move |__event_0, __event_1| __PagesViewMessage::OpenPageSearchHit(__event_0, __event_1)).clone(), (move || __PagesViewMessage::PostBlockCommentSubmit).clone(), (move |__event_0| __PagesViewMessage::PostThreadReply(__event_0)).clone(), (move |__event_0, __event_1| __PagesViewMessage::PagesPaneResized(__event_0, __event_1)).clone(), (move |__event_0, __event_1| __PagesViewMessage::SidebarResized(__event_0, __event_1)).clone(), (move |__event_0, __event_1| __PagesViewMessage::ResolveThreadSubmit(__event_0, __event_1)).clone(), (move || __PagesViewMessage::SearchPagesSubmit).clone(), (move |__event_0| __PagesViewMessage::SelectReplyThread(__event_0)).clone(), (move || __PagesViewMessage::ToggleBlockComments).clone(), (move || __PagesViewMessage::TogglePageCreate).clone(), (move || __PagesViewMessage::TogglePageMenu).clone(), (move || __PagesViewMessage::ToggleResolvedComments).clone(), (move |__event_0| __PagesViewMessage::ToggleThreadReplies(__event_0)).clone(), (move |__event_0| __PagesViewMessage::UseOrphanedCommentDraft(__event_0)).clone(), (move || __PagesViewMessage::WidenCommentScope).clone()));
#[cfg(test)]
drop(__ice_render_source);
__ice_rendered
}); if self.page_create_open { __children.push({
#[cfg(test)]
let __ice_render_source_location = ::ui_lang_runtime::testing::Location::new("src/ui/pages.ice", 92, 1, "rendered view node");
#[cfg(test)]
let __ice_render_source = ::ui_lang_runtime::testing::push_render_source(__ice_render_source_location);
let __ice_rendered: __IceElement<'_, __PagesViewMessage> = { let mut __children: ::std::vec::Vec<__IceElement<'_, __PagesViewMessage>> = ::std::vec::Vec::new(); __children.push({
#[cfg(test)]
let __ice_render_source_location = ::ui_lang_runtime::testing::Location::new("src/ui/pages.ice", 98, 1, "rendered view node");
#[cfg(test)]
let __ice_render_source = ::ui_lang_runtime::testing::push_render_source(__ice_render_source_location);
let __ice_rendered: __IceElement<'_, __PagesViewMessage> = { let __ice_node_scope = format!("{}/new-page", __ice_node_scope); ::ui_lang_guest::wire::Node::Input { options: ::ui_lang_guest::wire::InputOptions { label: ("New page title".to_owned()).to_string(), description: ::std::option::Option::None, disabled: ((self.loading || (self.busy || (!(self.host_error).is_empty()))) || (!self.connected)), padding: ::std::option::Option::Some(::ui_lang_guest::wire::Edges::all((6.2) as f32)), text_size: ::std::option::Option::Some((13.0) as f32), line_height: ::std::option::Option::Some((1.2) as f32), align: ::std::option::Option::None, font: ::std::option::Option::Some(::ui_lang_guest::wire::NamedFont { family: ::ui_lang_guest::wire::FontFamily::Named("Geist".into()), weight: ::ui_lang_guest::wire::Weight::Normal, stretch: ::ui_lang_guest::wire::FontStretch::Normal, style: ::ui_lang_guest::wire::FontStyle::Normal }) }, key: __ice_node_scope.clone(), placeholder: ::std::string::String::from("New page".to_owned()), value: (self.page_draft).to_string(), on_input: ::ui_lang_guest::slots::handler::<::std::string::String, __PagesViewMessage>(::std::boxed::Box::new({ let __route = __PagesViewMessage::__BindPageDraft as fn(::std::string::String) -> __PagesViewMessage; move |__sent: ::std::string::String| ::std::option::Option::Some(__route(__sent)) })), on_submit: ::std::option::Option::Some(::ui_lang_guest::slots::message((move || __PagesViewMessage::CreatePageSubmit)())), width: ::std::option::Option::Some(::ui_lang_guest::wire::Length::Fill), secure: (false), style: ::std::boxed::Box::new(::ui_lang_guest::wire::InputStyle { utility: ::ui_lang_guest::wire::InputFace { background: ::std::option::Option::Some({ let __color: ::iced::Color = __ice_palette.colors[3]; ::ui_lang_guest::wire::Rgba([__color.r, __color.g, __color.b, __color.a]) }), border: Some(::ui_lang_guest::wire::Border { color: ::std::option::Option::Some({ let __color: ::iced::Color = __ice_palette.colors[39]; ::ui_lang_guest::wire::Rgba([__color.r, __color.g, __color.b, __color.a]) }), width: ::std::option::Option::Some(1f32), radius: ::std::option::Option::Some([10f32; 4]) }), ..Default::default() }, focus_border: ::std::option::Option::Some({ let __color: ::iced::Color = __ice_palette.colors[42]; ::ui_lang_guest::wire::Rgba([__color.r, __color.g, __color.b, __color.a]) }), focused_hovered: ::std::option::Option::None, active: ::ui_lang_guest::wire::InputFace { icon: ::std::option::Option::None, background: ::std::option::Option::Some({ let __color: ::iced::Color = __ice_palette.colors[6]; ::ui_lang_guest::wire::Rgba([__color.r, __color.g, __color.b, __color.a]) }), border: ::std::option::Option::Some(::ui_lang_guest::wire::Border { color: ::std::option::Option::Some({ let __color: ::iced::Color = { let mut __color = __ice_palette.colors[4]; __color.a = 0.160000; __color }; ::ui_lang_guest::wire::Rgba([__color.r, __color.g, __color.b, __color.a]) }), width: ::std::option::Option::Some(((1.0) as f32).max(0.0).min(f32::MAX)), radius: ::std::option::Option::Some([((8.0) as f32).max(0.0).min(f32::MAX), ((8.0) as f32).max(0.0).min(f32::MAX), ((8.0) as f32).max(0.0).min(f32::MAX), ((8.0) as f32).max(0.0).min(f32::MAX)]) }), value: ::std::option::Option::Some({ let __color: ::iced::Color = __ice_palette.colors[4]; ::ui_lang_guest::wire::Rgba([__color.r, __color.g, __color.b, __color.a]) }), placeholder: ::std::option::Option::Some({ let __color: ::iced::Color = __ice_palette.colors[5]; ::ui_lang_guest::wire::Rgba([__color.r, __color.g, __color.b, __color.a]) }), selection: ::std::option::Option::Some({ let __color: ::iced::Color = { let mut __color = __ice_palette.colors[4]; __color.a = 0.180000; __color }; ::ui_lang_guest::wire::Rgba([__color.r, __color.g, __color.b, __color.a]) }) }, hovered: ::std::option::Option::Some(::ui_lang_guest::wire::InputFace { icon: ::std::option::Option::None, background: ::std::option::Option::Some({ let __color: ::iced::Color = __ice_palette.colors[55]; ::ui_lang_guest::wire::Rgba([__color.r, __color.g, __color.b, __color.a]) }), border: ::std::option::Option::Some(::ui_lang_guest::wire::Border { color: ::std::option::Option::Some({ let __color: ::iced::Color = { let mut __color = __ice_palette.colors[4]; __color.a = 0.210000; __color }; ::ui_lang_guest::wire::Rgba([__color.r, __color.g, __color.b, __color.a]) }), width: ::std::option::Option::None, radius: ::std::option::Option::None }), value: ::std::option::Option::None, placeholder: ::std::option::Option::None, selection: ::std::option::Option::None }), focused: ::std::option::Option::None, disabled: ::std::option::Option::Some(::ui_lang_guest::wire::InputFace { icon: ::std::option::Option::None, background: ::std::option::Option::Some({ let __color: ::iced::Color = { let mut __color = __ice_palette.colors[6]; __color.a = 0.540000; __color }; ::ui_lang_guest::wire::Rgba([__color.r, __color.g, __color.b, __color.a]) }), border: ::std::option::Option::None, value: ::std::option::Option::Some({ let __color: ::iced::Color = __ice_palette.colors[5]; ::ui_lang_guest::wire::Rgba([__color.r, __color.g, __color.b, __color.a]) }), placeholder: ::std::option::Option::None, selection: ::std::option::Option::None }) }) } };
#[cfg(test)]
drop(__ice_render_source);
__ice_rendered
}); __children.push({
#[cfg(test)]
let __ice_render_source_location = ::ui_lang_runtime::testing::Location::new("src/ui/pages.ice", 112, 1, "rendered view node");
#[cfg(test)]
let __ice_render_source = ::ui_lang_runtime::testing::push_render_source(__ice_render_source_location);
let __ice_rendered: __IceElement<'_, __PagesViewMessage> = ::ui_lang_guest::wire::Node::Button { checked: ::std::option::Option::None, expanded: ::std::option::Option::None, description: ::std::option::Option::None, key: format!("{}/@button:112", __ice_use_scope), content: ::ui_lang_guest::wire::ButtonContent::Child(::std::boxed::Box::new({
#[cfg(test)]
let __ice_render_source_location = ::ui_lang_runtime::testing::Location::new("src/ui/pages.ice", 120, 1, "rendered view node");
#[cfg(test)]
let __ice_render_source = ::ui_lang_runtime::testing::push_render_source(__ice_render_source_location);
let __ice_rendered: __IceElement<'_, __PagesViewMessage> = ::ui_lang_guest::wire::Node::Container { shadow: ::ui_lang_guest::wire::Shadow { color: ::std::option::Option::None, x: ::std::option::Option::None, y: ::std::option::Option::None, blur: ::std::option::Option::None }, max_width: ::std::option::Option::None, max_height: ::std::option::Option::None, clip: false, key: format!("{}/@container:120", __ice_use_scope), width: ::std::option::Option::Some(::ui_lang_guest::wire::Length::Fill), height: ::std::option::Option::Some(::ui_lang_guest::wire::Length::Fill), padding: ::std::option::Option::None, align_x: ::std::option::Option::Some(::ui_lang_guest::wire::AlignX::Center), align_y: ::std::option::Option::Some(::ui_lang_guest::wire::AlignY::Center), background: (::std::option::Option::None).map(::ui_lang_guest::wire::Background::Color), border: ::std::option::Option::None, snap: ::std::option::Option::None, content: ::std::boxed::Box::new({
#[cfg(test)]
let __ice_render_source_location = ::ui_lang_runtime::testing::Location::new("src/ui/pages.ice", 126, 1, "rendered view node");
#[cfg(test)]
let __ice_render_source = ::ui_lang_runtime::testing::push_render_source(__ice_render_source_location);
let __ice_rendered: __IceElement<'_, __PagesViewMessage> = ::ui_lang_guest::wire::Node::Text { options: ::ui_lang_guest::wire::TextOptions { height: ::std::option::Option::None, align_y: ::std::option::Option::None, line_height: ::std::option::Option::None, shaping: ::std::option::Option::None, wrapping: ::std::option::Option::None, tracking: 0.0f32, font: ::std::option::Option::Some(::ui_lang_guest::wire::NamedFont { family: ::ui_lang_guest::wire::FontFamily::Named("Geist".into()), weight: ::ui_lang_guest::wire::Weight::Normal, stretch: ::ui_lang_guest::wire::FontStretch::Normal, style: ::ui_lang_guest::wire::FontStyle::Normal }) }, key: format!("{}/@text:126", __ice_use_scope), size: ::std::option::Option::Some(((14.0) as f32).max(f32::EPSILON).min(f32::MAX)), color: ::std::option::Option::None, font: ::ui_lang_guest::wire::Font { monospace: false, weight: ::ui_lang_guest::wire::Weight::Normal }, width: ::std::option::Option::None, align_x: ::std::option::Option::None, content: ("+".to_owned()).to_string() };
#[cfg(test)]
drop(__ice_render_source);
__ice_rendered
}) };
#[cfg(test)]
drop(__ice_render_source);
__ice_rendered
})), label: ::std::option::Option::Some(::std::string::String::from("Create page".to_owned())), on_press: if ((((self.loading || (self.busy || (!(self.host_error).is_empty()))) || (!self.connected)) || ((self.page_draft).trim().to_owned()).is_empty())) { ::std::option::Option::None } else { ::std::option::Option::Some(::ui_lang_guest::slots::message((move || __PagesViewMessage::CreatePageSubmit)())) }, width: ::std::option::Option::Some(::ui_lang_guest::wire::Length::Fixed((28.0) as f32)), height: ::std::option::Option::Some(::ui_lang_guest::wire::Length::Fixed((28.0) as f32)), padding: ::std::option::Option::Some(::ui_lang_guest::wire::Edges::all((0.0) as f32)), style: ::ui_lang_guest::wire::ButtonStyle { preset: ::ui_lang_guest::wire::ButtonPreset::Primary, recipe: Some(::ui_lang_guest::wire::ButtonRecipe { base: ::ui_lang_guest::wire::Face { background: ::std::option::Option::Some({ let __color: ::iced::Color = ::iced::Color::from_rgba8(0, 0, 0, 0.000000); ::ui_lang_guest::wire::Rgba([__color.r, __color.g, __color.b, __color.a]) }), text: ::std::option::Option::Some({ let __color: ::iced::Color = __ice_palette.colors[4]; ::ui_lang_guest::wire::Rgba([__color.r, __color.g, __color.b, __color.a]) }), border: ::std::option::Option::Some(::ui_lang_guest::wire::Border { color: ::std::option::Option::None, width: ::std::option::Option::None, radius: ::std::option::Option::Some([7.0; 4]) }) }, hover_background: ::std::option::Option::Some({ let __color: ::iced::Color = __ice_palette.colors[14]; ::ui_lang_guest::wire::Rgba([__color.r, __color.g, __color.b, __color.a]) }), pressed_background: ::std::option::Option::Some({ let __color: ::iced::Color = __ice_palette.colors[39]; ::ui_lang_guest::wire::Rgba([__color.r, __color.g, __color.b, __color.a]) }), disabled_background: ::std::option::Option::None, disabled_text: ::std::option::Option::None, disabled_opacity: ::std::option::Option::Some(0.5f32), focus_ring: ::std::option::Option::Some({ let __color: ::iced::Color = __ice_palette.colors[42]; ::ui_lang_guest::wire::Rgba([__color.r, __color.g, __color.b, __color.a]) }), text_size: ::std::option::Option::Some(13.5f32), line_height: ::std::option::Option::None, font: Some(::ui_lang_guest::wire::NamedFont { family: ::ui_lang_guest::wire::FontFamily::Named("Geist".into()), weight: ::ui_lang_guest::wire::Weight::Normal, stretch: ::ui_lang_guest::wire::FontStretch::Normal, style: ::ui_lang_guest::wire::FontStyle::Normal }) }), active: ::ui_lang_guest::wire::Face::default(), hovered: ::std::option::Option::None, pressed: ::std::option::Option::None, disabled: ::std::option::Option::None } };
#[cfg(test)]
drop(__ice_render_source);
__ice_rendered
}); ::ui_lang_guest::wire::Node::Linear { max_width: ::std::option::Option::None, clip: false, key: format!("{}/@layout:92", __ice_use_scope), wrap: None, axis: ::ui_lang_guest::wire::Axis::Row, spacing: ::std::option::Option::Some((5.0) as f32), padding: ::std::option::Option::None, width: ::std::option::Option::Some(::ui_lang_guest::wire::Length::Fill), height: ::std::option::Option::Some(::ui_lang_guest::wire::Length::Fixed((28.0) as f32)), align: ::std::option::Option::Some(::ui_lang_guest::wire::AlignX::Center), background: ::std::option::Option::None, border: ::std::option::Option::None, children: __children } };
#[cfg(test)]
drop(__ice_render_source);
__ice_rendered
}); } __children.push({
#[cfg(test)]
let __ice_render_source_location = ::ui_lang_runtime::testing::Location::new("src/ui/pages.ice", 127, 1, "rendered view node");
#[cfg(test)]
let __ice_render_source = ::ui_lang_runtime::testing::push_render_source(__ice_render_source_location);
let __ice_rendered: __IceElement<'_, __PagesViewMessage> = ::ui_lang_guest::wire::Node::Scroll { on_scroll: ::std::option::Option::None, virtual_rows: false, key: format!("{}/@layout:127", __ice_use_scope), direction: ::ui_lang_guest::wire::ScrollDirection::Vertical, width: ::std::option::Option::Some(::ui_lang_guest::wire::Length::Fill), height: ::std::option::Option::Some(::ui_lang_guest::wire::Length::Fill), bar_hidden: false, bar_width: ::std::option::Option::None, bar_margin: ::std::option::Option::None, scroller_width: ::std::option::Option::None, bar_spacing: ::std::option::Option::None, anchor_x: ::ui_lang_guest::wire::ScrollAnchor::Start, anchor_y: ::ui_lang_guest::wire::ScrollAnchor::Start, auto_scroll: (false), background: ::std::option::Option::None, border: ::std::option::Option::None, content: ::std::boxed::Box::new({
#[cfg(test)]
let __ice_render_source_location = ::ui_lang_runtime::testing::Location::new("src/ui/pages.ice", 132, 1, "rendered view node");
#[cfg(test)]
let __ice_render_source = ::ui_lang_runtime::testing::push_render_source(__ice_render_source_location);
let __ice_rendered: __IceElement<'_, __PagesViewMessage> = { let mut __children: ::std::vec::Vec<__IceElement<'_, __PagesViewMessage>> = ::std::vec::Vec::new(); for (__ice_index, page) in self.pages.iter().enumerate() { let __for_scope = format!("{}/@for:754({})", __ice_use_scope, __ice_index); __children.push({
#[cfg(test)]
let __ice_render_source_location = ::ui_lang_runtime::testing::Location::new("src/ui/pages.ice", 134, 1, "rendered view node");
#[cfg(test)]
let __ice_render_source = ::ui_lang_runtime::testing::push_render_source(__ice_render_source_location);
let __ice_rendered: __IceElement<'_, __PagesViewMessage> = ::ui_lang_runtime::grow_stack(|| self.__ice_component_use_3(__ice_palette, format!("{}/PageButton@755", __for_scope), (move |__event_0| __PagesViewMessage::ChoosePage(__event_0)).clone(), page.clone(), (page.id == self.active_page)));
#[cfg(test)]
drop(__ice_render_source);
__ice_rendered
}); } ::ui_lang_guest::wire::Node::Linear { max_width: ::std::option::Option::None, clip: false, key: format!("{}/@layout:132", __ice_use_scope), wrap: None, axis: ::ui_lang_guest::wire::Axis::Column, spacing: ::std::option::Option::Some((2.0) as f32), padding: ::std::option::Option::None, width: ::std::option::Option::Some(::ui_lang_guest::wire::Length::Fill), height: ::std::option::Option::None, align: ::std::option::Option::None, background: ::std::option::Option::None, border: ::std::option::Option::None, children: __children } };
#[cfg(test)]
drop(__ice_render_source);
__ice_rendered
}) };
#[cfg(test)]
drop(__ice_render_source);
__ice_rendered
}); ::ui_lang_guest::wire::Node::Linear { max_width: ::std::option::Option::None, clip: false, key: format!("{}/@layout:40", __ice_use_scope), wrap: None, axis: ::ui_lang_guest::wire::Axis::Column, spacing: ::std::option::Option::Some((0.0) as f32), padding: ::std::option::Option::None, width: ::std::option::Option::Some(::ui_lang_guest::wire::Length::Fill), height: ::std::option::Option::Some(::ui_lang_guest::wire::Length::Fill), align: ::std::option::Option::None, background: ::std::option::Option::None, border: ::std::option::Option::None, children: __children } };
#[cfg(test)]
drop(__ice_render_source);
__ice_rendered
}) } };
#[cfg(test)]
drop(__ice_render_source);
__ice_rendered
}); __children.push({
#[cfg(test)]
let __ice_render_source_location = ::ui_lang_runtime::testing::Location::new("src/ui/pages.ice", 141, 1, "rendered view node");
#[cfg(test)]
let __ice_render_source = ::ui_lang_runtime::testing::push_render_source(__ice_render_source_location);
let __ice_rendered: __IceElement<'_, __PagesViewMessage> = { let __ice_node_scope = format!("{}/sidebar-resize", __ice_use_scope); ::ui_lang_guest::wire::Node::ResizeHandle { key: __ice_node_scope.clone(), on_press: ::std::option::Option::None, on_release: ::std::option::Option::None, on_drag: ::std::option::Option::Some(::ui_lang_guest::slots::handler::<(f64, f64), __PagesViewMessage>(::std::boxed::Box::new({ let __route = {  { let __route_callback = (move |__event_0, __event_1| __PagesViewMessage::SidebarResized(__event_0, __event_1)).clone(); move |__delta: (f64, f64)| (__route_callback)(__delta.0, __delta.1) } }; move |__sent: (f64, f64)| ::std::option::Option::Some(__route(__sent)) }))), cursor: ::std::option::Option::Some(::ui_lang_guest::wire::mouse::Cursor::ResizingHorizontally), content: ::std::boxed::Box::new({
#[cfg(test)]
let __ice_render_source_location = ::ui_lang_runtime::testing::Location::new("src/ui/pages.ice", 142, 1, "rendered view node");
#[cfg(test)]
let __ice_render_source = ::ui_lang_runtime::testing::push_render_source(__ice_render_source_location);
let __ice_rendered: __IceElement<'_, __PagesViewMessage> = { let __ice_node_scope = format!("{}/sidebar-divider", __ice_node_scope); ::ui_lang_guest::wire::Node::Container { shadow: ::ui_lang_guest::wire::Shadow { color: ::std::option::Option::None, x: ::std::option::Option::None, y: ::std::option::Option::None, blur: ::std::option::Option::None }, max_width: ::std::option::Option::None, max_height: ::std::option::Option::None, clip: false, key: __ice_node_scope.clone(), width: ::std::option::Option::Some(::ui_lang_guest::wire::Length::Fixed((10.0) as f32)), height: ::std::option::Option::Some(::ui_lang_guest::wire::Length::Fill), padding: ::std::option::Option::None, align_x: ::std::option::Option::Some(::ui_lang_guest::wire::AlignX::Left), align_y: ::std::option::Option::None, background: (::std::option::Option::None).map(::ui_lang_guest::wire::Background::Color), border: ::std::option::Option::None, snap: ::std::option::Option::None, content: ::std::boxed::Box::new({
#[cfg(test)]
let __ice_render_source_location = ::ui_lang_runtime::testing::Location::new("src/ui/pages.ice", 147, 1, "rendered view node");
#[cfg(test)]
let __ice_render_source = ::ui_lang_runtime::testing::push_render_source(__ice_render_source_location);
let __ice_rendered: __IceElement<'_, __PagesViewMessage> = ::ui_lang_guest::wire::Node::Container { shadow: ::ui_lang_guest::wire::Shadow { color: ::std::option::Option::None, x: ::std::option::Option::None, y: ::std::option::Option::None, blur: ::std::option::Option::None }, max_width: ::std::option::Option::None, max_height: ::std::option::Option::None, clip: false, key: format!("{}/@container:147", __ice_use_scope), width: ::std::option::Option::Some(::ui_lang_guest::wire::Length::Fixed((1.0) as f32)), height: ::std::option::Option::Some(::ui_lang_guest::wire::Length::Fill), padding: ::std::option::Option::None, align_x: ::std::option::Option::None, align_y: ::std::option::Option::None, background: (::std::option::Option::Some({ let __color: ::iced::Color = __ice_palette.colors[60]; ::ui_lang_guest::wire::Rgba([__color.r, __color.g, __color.b, __color.a]) })).map(::ui_lang_guest::wire::Background::Color), border: ::std::option::Option::None, snap: ::std::option::Option::None, content: ::std::boxed::Box::new({
#[cfg(test)]
let __ice_render_source_location = ::ui_lang_runtime::testing::Location::new("src/ui/pages.ice", 152, 1, "rendered view node");
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
}) } };
#[cfg(test)]
drop(__ice_render_source);
__ice_rendered
}) } };
#[cfg(test)]
drop(__ice_render_source);
__ice_rendered
}); __children.push({
#[cfg(test)]
let __ice_render_source_location = ::ui_lang_runtime::testing::Location::new("src/ui/pages.ice", 153, 1, "rendered view node");
#[cfg(test)]
let __ice_render_source = ::ui_lang_runtime::testing::push_render_source(__ice_render_source_location);
let __ice_rendered: __IceElement<'_, __PagesViewMessage> = { let mut __children: ::std::vec::Vec<__IceElement<'_, __PagesViewMessage>> = ::std::vec::Vec::new(); __children.push({
#[cfg(test)]
let __ice_render_source_location = ::ui_lang_runtime::testing::Location::new("src/ui/pages.ice", 154, 1, "rendered view node");
#[cfg(test)]
let __ice_render_source = ::ui_lang_runtime::testing::push_render_source(__ice_render_source_location);
let __ice_rendered: __IceElement<'_, __PagesViewMessage> = { let mut __children: ::std::vec::Vec<__IceElement<'_, __PagesViewMessage>> = ::std::vec::Vec::new(); if (!(self.host_error).is_empty()) { __children.push({
#[cfg(test)]
let __ice_render_source_location = ::ui_lang_runtime::testing::Location::new("src/ui/pages.ice", 156, 1, "rendered view node");
#[cfg(test)]
let __ice_render_source = ::ui_lang_runtime::testing::push_render_source(__ice_render_source_location);
let __ice_rendered: __IceElement<'_, __PagesViewMessage> = ::ui_lang_guest::wire::Node::Container { shadow: ::ui_lang_guest::wire::Shadow { color: ::std::option::Option::None, x: ::std::option::Option::None, y: ::std::option::Option::None, blur: ::std::option::Option::None }, max_width: ::std::option::Option::None, max_height: ::std::option::Option::None, clip: false, key: format!("{}/@container:156", __ice_use_scope), width: ::std::option::Option::Some(::ui_lang_guest::wire::Length::Fill), height: ::std::option::Option::None, padding: ::std::option::Option::Some(::ui_lang_guest::wire::Edges { top: (12.0) as f32, right: (12.0) as f32, bottom: (12.0) as f32, left: (12.0) as f32 }), align_x: ::std::option::Option::None, align_y: ::std::option::Option::None, background: (::std::option::Option::Some({ let __color: ::iced::Color = __ice_palette.colors[22]; ::ui_lang_guest::wire::Rgba([__color.r, __color.g, __color.b, __color.a]) })).map(::ui_lang_guest::wire::Background::Color), border: ::std::option::Option::None, snap: ::std::option::Option::None, content: ::std::boxed::Box::new({
#[cfg(test)]
let __ice_render_source_location = ::ui_lang_runtime::testing::Location::new("src/ui/pages.ice", 157, 1, "rendered view node");
#[cfg(test)]
let __ice_render_source = ::ui_lang_runtime::testing::push_render_source(__ice_render_source_location);
let __ice_rendered: __IceElement<'_, __PagesViewMessage> = { let mut __children: ::std::vec::Vec<__IceElement<'_, __PagesViewMessage>> = ::std::vec::Vec::new(); __children.push({
#[cfg(test)]
let __ice_render_source_location = ::ui_lang_runtime::testing::Location::new("src/ui/pages.ice", 158, 1, "rendered view node");
#[cfg(test)]
let __ice_render_source = ::ui_lang_runtime::testing::push_render_source(__ice_render_source_location);
let __ice_rendered: __IceElement<'_, __PagesViewMessage> = ::ui_lang_guest::wire::Node::Text { options: ::ui_lang_guest::wire::TextOptions { height: ::std::option::Option::None, align_y: ::std::option::Option::None, line_height: ::std::option::Option::None, shaping: ::std::option::Option::None, wrapping: ::std::option::Option::None, tracking: 0.0f32, font: ::std::option::Option::Some(::ui_lang_guest::wire::NamedFont { family: ::ui_lang_guest::wire::FontFamily::Named("Geist".into()), weight: ::ui_lang_guest::wire::Weight::Normal, stretch: ::ui_lang_guest::wire::FontStretch::Normal, style: ::ui_lang_guest::wire::FontStyle::Normal }) }, key: format!("{}/@text:158", __ice_use_scope), size: ::std::option::Option::Some(((13.0) as f32).max(f32::EPSILON).min(f32::MAX)), color: ::std::option::Option::Some({ let __color: ::iced::Color = __ice_palette.colors[20]; ::ui_lang_guest::wire::Rgba([__color.r, __color.g, __color.b, __color.a]) }), font: ::ui_lang_guest::wire::Font { monospace: false, weight: ::ui_lang_guest::wire::Weight::Normal }, width: ::std::option::Option::None, align_x: ::std::option::Option::None, content: ("Pages could not load".to_owned()).to_string() };
#[cfg(test)]
drop(__ice_render_source);
__ice_rendered
}); __children.push({
#[cfg(test)]
let __ice_render_source_location = ::ui_lang_runtime::testing::Location::new("src/ui/pages.ice", 159, 1, "rendered view node");
#[cfg(test)]
let __ice_render_source = ::ui_lang_runtime::testing::push_render_source(__ice_render_source_location);
let __ice_rendered: __IceElement<'_, __PagesViewMessage> = ::ui_lang_guest::wire::Node::Text { options: ::ui_lang_guest::wire::TextOptions { height: ::std::option::Option::None, align_y: ::std::option::Option::None, line_height: ::std::option::Option::None, shaping: ::std::option::Option::None, wrapping: ::std::option::Option::None, tracking: 0.0f32, font: ::std::option::Option::Some(::ui_lang_guest::wire::NamedFont { family: ::ui_lang_guest::wire::FontFamily::Named("Geist".into()), weight: ::ui_lang_guest::wire::Weight::Normal, stretch: ::ui_lang_guest::wire::FontStretch::Normal, style: ::ui_lang_guest::wire::FontStyle::Normal }) }, key: format!("{}/@text:159", __ice_use_scope), size: ::std::option::Option::Some(((12.0) as f32).max(f32::EPSILON).min(f32::MAX)), color: ::std::option::Option::Some({ let __color: ::iced::Color = __ice_palette.colors[20]; ::ui_lang_guest::wire::Rgba([__color.r, __color.g, __color.b, __color.a]) }), font: ::ui_lang_guest::wire::Font { monospace: false, weight: ::ui_lang_guest::wire::Weight::Normal }, width: ::std::option::Option::None, align_x: ::std::option::Option::None, content: (self.host_error.to_owned()).to_string() };
#[cfg(test)]
drop(__ice_render_source);
__ice_rendered
}); ::ui_lang_guest::wire::Node::Linear { max_width: ::std::option::Option::None, clip: false, key: format!("{}/@layout:157", __ice_use_scope), wrap: None, axis: ::ui_lang_guest::wire::Axis::Column, spacing: ::std::option::Option::Some((4.0) as f32), padding: ::std::option::Option::None, width: ::std::option::Option::Some(::ui_lang_guest::wire::Length::Fill), height: ::std::option::Option::None, align: ::std::option::Option::None, background: ::std::option::Option::None, border: ::std::option::Option::None, children: __children } };
#[cfg(test)]
drop(__ice_render_source);
__ice_rendered
}) };
#[cfg(test)]
drop(__ice_render_source);
__ice_rendered
}); } if (self.connected && (!(self.active_page).is_empty())) { __children.push({
#[cfg(test)]
let __ice_render_source_location = ::ui_lang_runtime::testing::Location::new("src/ui/pages.ice", 163, 1, "rendered view node");
#[cfg(test)]
let __ice_render_source = ::ui_lang_runtime::testing::push_render_source(__ice_render_source_location);
let __ice_rendered: __IceElement<'_, __PagesViewMessage> = { let mut __children: ::std::vec::Vec<__IceElement<'_, __PagesViewMessage>> = ::std::vec::Vec::new(); __children.push({
#[cfg(test)]
let __ice_render_source_location = ::ui_lang_runtime::testing::Location::new("src/ui/pages.ice", 164, 1, "rendered view node");
#[cfg(test)]
let __ice_render_source = ::ui_lang_runtime::testing::push_render_source(__ice_render_source_location);
let __ice_rendered: __IceElement<'_, __PagesViewMessage> = ::ui_lang_guest::wire::Node::Container { shadow: ::ui_lang_guest::wire::Shadow { color: ::std::option::Option::None, x: ::std::option::Option::None, y: ::std::option::Option::None, blur: ::std::option::Option::None }, max_width: ::std::option::Option::None, max_height: ::std::option::Option::None, clip: false, key: format!("{}/@container:164", __ice_use_scope), width: ::std::option::Option::Some(::ui_lang_guest::wire::Length::Fill), height: ::std::option::Option::Some(::ui_lang_guest::wire::Length::Fixed((50.0) as f32)), padding: ::std::option::Option::Some(::ui_lang_guest::wire::Edges { top: (0.0) as f32, right: (22.0) as f32, bottom: (0.0) as f32, left: (22.0) as f32 }), align_x: ::std::option::Option::None, align_y: ::std::option::Option::None, background: (::std::option::Option::None).map(::ui_lang_guest::wire::Background::Color), border: ::std::option::Option::None, snap: ::std::option::Option::None, content: ::std::boxed::Box::new({
#[cfg(test)]
let __ice_render_source_location = ::ui_lang_runtime::testing::Location::new("src/ui/pages.ice", 170, 1, "rendered view node");
#[cfg(test)]
let __ice_render_source = ::ui_lang_runtime::testing::push_render_source(__ice_render_source_location);
let __ice_rendered: __IceElement<'_, __PagesViewMessage> = { let mut __children: ::std::vec::Vec<__IceElement<'_, __PagesViewMessage>> = ::std::vec::Vec::new(); __children.push({
#[cfg(test)]
let __ice_render_source_location = ::ui_lang_runtime::testing::Location::new("src/ui/pages.ice", 185, 1, "rendered view node");
#[cfg(test)]
let __ice_render_source = ::ui_lang_runtime::testing::push_render_source(__ice_render_source_location);
let __ice_rendered: __IceElement<'_, __PagesViewMessage> = ::ui_lang_guest::wire::Node::Container { shadow: ::ui_lang_guest::wire::Shadow { color: ::std::option::Option::None, x: ::std::option::Option::None, y: ::std::option::Option::None, blur: ::std::option::Option::None }, max_width: ::std::option::Option::None, max_height: ::std::option::Option::None, clip: true, key: format!("{}/@container:185", __ice_use_scope), width: ::std::option::Option::Some(::ui_lang_guest::wire::Length::Fill), height: ::std::option::Option::None, padding: ::std::option::Option::None, align_x: ::std::option::Option::None, align_y: ::std::option::Option::None, background: (::std::option::Option::None).map(::ui_lang_guest::wire::Background::Color), border: ::std::option::Option::None, snap: ::std::option::Option::None, content: ::std::boxed::Box::new({
#[cfg(test)]
let __ice_render_source_location = ::ui_lang_runtime::testing::Location::new("src/ui/pages.ice", 186, 1, "rendered view node");
#[cfg(test)]
let __ice_render_source = ::ui_lang_runtime::testing::push_render_source(__ice_render_source_location);
let __ice_rendered: __IceElement<'_, __PagesViewMessage> = ::ui_lang_guest::wire::Node::Text { options: ::ui_lang_guest::wire::TextOptions { height: ::std::option::Option::None, align_y: ::std::option::Option::None, line_height: ::std::option::Option::None, shaping: ::std::option::Option::None, wrapping: ::std::option::Option::Some(::ui_lang_guest::wire::Wrapping::None), tracking: 0.0f32, font: ::std::option::Option::Some(::ui_lang_guest::wire::NamedFont { family: ::ui_lang_guest::wire::FontFamily::Named("Geist".into()), weight: ::ui_lang_guest::wire::Weight::Semibold, stretch: ::ui_lang_guest::wire::FontStretch::Normal, style: ::ui_lang_guest::wire::FontStyle::Normal }) }, key: format!("{}/@text:186", __ice_use_scope), size: ::std::option::Option::Some(((13.5) as f32).max(f32::EPSILON).min(f32::MAX)), color: ::std::option::Option::Some({ let __color: ::iced::Color = __ice_palette.colors[4]; ::ui_lang_guest::wire::Rgba([__color.r, __color.g, __color.b, __color.a]) }), font: ::ui_lang_guest::wire::Font { monospace: false, weight: ::ui_lang_guest::wire::Weight::Normal }, width: ::std::option::Option::None, align_x: ::std::option::Option::None, content: (self.active_page_title.to_owned()).to_string() };
#[cfg(test)]
drop(__ice_render_source);
__ice_rendered
}) };
#[cfg(test)]
drop(__ice_render_source);
__ice_rendered
}); if (!(self.active_page_parent).is_empty()) { __children.push({
#[cfg(test)]
let __ice_render_source_location = ::ui_lang_runtime::testing::Location::new("src/ui/pages.ice", 196, 1, "rendered view node");
#[cfg(test)]
let __ice_render_source = ::ui_lang_runtime::testing::push_render_source(__ice_render_source_location);
let __ice_rendered: __IceElement<'_, __PagesViewMessage> = ::ui_lang_guest::wire::Node::Text { options: ::ui_lang_guest::wire::TextOptions { height: ::std::option::Option::None, align_y: ::std::option::Option::None, line_height: ::std::option::Option::None, shaping: ::std::option::Option::None, wrapping: ::std::option::Option::Some(::ui_lang_guest::wire::Wrapping::None), tracking: 0.0f32, font: ::std::option::Option::Some(::ui_lang_guest::wire::NamedFont { family: ::ui_lang_guest::wire::FontFamily::Named("Geist Mono".into()), weight: ::ui_lang_guest::wire::Weight::Normal, stretch: ::ui_lang_guest::wire::FontStretch::Normal, style: ::ui_lang_guest::wire::FontStyle::Normal }) }, key: format!("{}/@text:196", __ice_use_scope), size: ::std::option::Option::Some(((11.0) as f32).max(f32::EPSILON).min(f32::MAX)), color: ::std::option::Option::Some({ let __color: ::iced::Color = __ice_palette.colors[72]; ::ui_lang_guest::wire::Rgba([__color.r, __color.g, __color.b, __color.a]) }), font: ::ui_lang_guest::wire::Font { monospace: false, weight: ::ui_lang_guest::wire::Weight::Normal }, width: ::std::option::Option::None, align_x: ::std::option::Option::None, content: (self.active_page_parent.to_owned()).to_string() };
#[cfg(test)]
drop(__ice_render_source);
__ice_rendered
}); } __children.push({
#[cfg(test)]
let __ice_render_source_location = ::ui_lang_runtime::testing::Location::new("src/ui/pages.ice", 202, 1, "rendered view node");
#[cfg(test)]
let __ice_render_source = ::ui_lang_runtime::testing::push_render_source(__ice_render_source_location);
let __ice_rendered: __IceElement<'_, __PagesViewMessage> = { let __ice_node_scope = format!("{}/page-search", __ice_use_scope); ::ui_lang_guest::wire::Node::Input { options: ::ui_lang_guest::wire::InputOptions { label: ("Search pages".to_owned()).to_string(), description: ::std::option::Option::None, disabled: (((!(self.host_error).is_empty()) || (!self.connected)) || self.page_searching), padding: ::std::option::Option::Some(::ui_lang_guest::wire::Edges::all((6.2) as f32)), text_size: ::std::option::Option::Some((13.0) as f32), line_height: ::std::option::Option::Some((1.2) as f32), align: ::std::option::Option::None, font: ::std::option::Option::Some(::ui_lang_guest::wire::NamedFont { family: ::ui_lang_guest::wire::FontFamily::Named("Geist".into()), weight: ::ui_lang_guest::wire::Weight::Normal, stretch: ::ui_lang_guest::wire::FontStretch::Normal, style: ::ui_lang_guest::wire::FontStyle::Normal }) }, key: __ice_node_scope.clone(), placeholder: ::std::string::String::from("Search pages…".to_owned()), value: (self.page_search_draft).to_string(), on_input: ::ui_lang_guest::slots::handler::<::std::string::String, __PagesViewMessage>(::std::boxed::Box::new({ let __route = __PagesViewMessage::__BindPageSearchDraft as fn(::std::string::String) -> __PagesViewMessage; move |__sent: ::std::string::String| ::std::option::Option::Some(__route(__sent)) })), on_submit: ::std::option::Option::Some(::ui_lang_guest::slots::message((move || __PagesViewMessage::SearchPagesSubmit)())), width: ::std::option::Option::Some(::ui_lang_guest::wire::Length::Fixed((190.0) as f32)), secure: (false), style: ::std::boxed::Box::new(::ui_lang_guest::wire::InputStyle { utility: ::ui_lang_guest::wire::InputFace { background: ::std::option::Option::Some({ let __color: ::iced::Color = __ice_palette.colors[3]; ::ui_lang_guest::wire::Rgba([__color.r, __color.g, __color.b, __color.a]) }), border: Some(::ui_lang_guest::wire::Border { color: ::std::option::Option::Some({ let __color: ::iced::Color = __ice_palette.colors[39]; ::ui_lang_guest::wire::Rgba([__color.r, __color.g, __color.b, __color.a]) }), width: ::std::option::Option::Some(1f32), radius: ::std::option::Option::Some([10f32; 4]) }), ..Default::default() }, focus_border: ::std::option::Option::Some({ let __color: ::iced::Color = __ice_palette.colors[42]; ::ui_lang_guest::wire::Rgba([__color.r, __color.g, __color.b, __color.a]) }), focused_hovered: ::std::option::Option::None, active: ::ui_lang_guest::wire::InputFace { icon: ::std::option::Option::None, background: ::std::option::Option::Some({ let __color: ::iced::Color = ::iced::Color::from_rgba8(0, 0, 0, 0.000000); ::ui_lang_guest::wire::Rgba([__color.r, __color.g, __color.b, __color.a]) }), border: ::std::option::Option::Some(::ui_lang_guest::wire::Border { color: ::std::option::Option::Some({ let __color: ::iced::Color = ::iced::Color::from_rgba8(0, 0, 0, 0.000000); ::ui_lang_guest::wire::Rgba([__color.r, __color.g, __color.b, __color.a]) }), width: ::std::option::Option::Some(((1.0) as f32).max(0.0).min(f32::MAX)), radius: ::std::option::Option::Some([((7.0) as f32).max(0.0).min(f32::MAX), ((7.0) as f32).max(0.0).min(f32::MAX), ((7.0) as f32).max(0.0).min(f32::MAX), ((7.0) as f32).max(0.0).min(f32::MAX)]) }), value: ::std::option::Option::Some({ let __color: ::iced::Color = __ice_palette.colors[4]; ::ui_lang_guest::wire::Rgba([__color.r, __color.g, __color.b, __color.a]) }), placeholder: ::std::option::Option::Some({ let __color: ::iced::Color = __ice_palette.colors[5]; ::ui_lang_guest::wire::Rgba([__color.r, __color.g, __color.b, __color.a]) }), selection: ::std::option::Option::Some({ let __color: ::iced::Color = { let mut __color = __ice_palette.colors[4]; __color.a = 0.180000; __color }; ::ui_lang_guest::wire::Rgba([__color.r, __color.g, __color.b, __color.a]) }) }, hovered: ::std::option::Option::Some(::ui_lang_guest::wire::InputFace { icon: ::std::option::Option::None, background: ::std::option::Option::Some({ let __color: ::iced::Color = { let mut __color = __ice_palette.colors[4]; __color.a = 0.050000; __color }; ::ui_lang_guest::wire::Rgba([__color.r, __color.g, __color.b, __color.a]) }), border: ::std::option::Option::Some(::ui_lang_guest::wire::Border { color: ::std::option::Option::Some({ let __color: ::iced::Color = { let mut __color = __ice_palette.colors[4]; __color.a = 0.080000; __color }; ::ui_lang_guest::wire::Rgba([__color.r, __color.g, __color.b, __color.a]) }), width: ::std::option::Option::None, radius: ::std::option::Option::None }), value: ::std::option::Option::None, placeholder: ::std::option::Option::None, selection: ::std::option::Option::None }), focused: ::std::option::Option::Some(::ui_lang_guest::wire::InputFace { icon: ::std::option::Option::None, background: ::std::option::Option::Some({ let __color: ::iced::Color = { let mut __color = __ice_palette.colors[4]; __color.a = 0.050000; __color }; ::ui_lang_guest::wire::Rgba([__color.r, __color.g, __color.b, __color.a]) }), border: ::std::option::Option::Some(::ui_lang_guest::wire::Border { color: ::std::option::Option::Some({ let __color: ::iced::Color = __ice_palette.colors[42]; ::ui_lang_guest::wire::Rgba([__color.r, __color.g, __color.b, __color.a]) }), width: ::std::option::Option::None, radius: ::std::option::Option::None }), value: ::std::option::Option::None, placeholder: ::std::option::Option::None, selection: ::std::option::Option::None }), disabled: ::std::option::Option::Some(::ui_lang_guest::wire::InputFace { icon: ::std::option::Option::None, background: ::std::option::Option::None, border: ::std::option::Option::None, value: ::std::option::Option::Some({ let __color: ::iced::Color = __ice_palette.colors[5]; ::ui_lang_guest::wire::Rgba([__color.r, __color.g, __color.b, __color.a]) }), placeholder: ::std::option::Option::None, selection: ::std::option::Option::None }) }) } };
#[cfg(test)]
drop(__ice_render_source);
__ice_rendered
}); if ((!((self.page_search_draft).trim().to_owned()).is_empty()) || (!(self.page_search_hits).is_empty())) { __children.push({
#[cfg(test)]
let __ice_render_source_location = ::ui_lang_runtime::testing::Location::new("src/ui/pages.ice", 224, 1, "rendered view node");
#[cfg(test)]
let __ice_render_source = ::ui_lang_runtime::testing::push_render_source(__ice_render_source_location);
let __ice_rendered: __IceElement<'_, __PagesViewMessage> = ::ui_lang_guest::wire::Node::Button { checked: ::std::option::Option::None, expanded: ::std::option::Option::None, description: ::std::option::Option::None, key: format!("{}/@button:224", __ice_use_scope), content: ::ui_lang_guest::wire::ButtonContent::Child(::std::boxed::Box::new({
#[cfg(test)]
let __ice_render_source_location = ::ui_lang_runtime::testing::Location::new("src/ui/pages.ice", 232, 1, "rendered view node");
#[cfg(test)]
let __ice_render_source = ::ui_lang_runtime::testing::push_render_source(__ice_render_source_location);
let __ice_rendered: __IceElement<'_, __PagesViewMessage> = ::ui_lang_guest::wire::Node::Container { shadow: ::ui_lang_guest::wire::Shadow { color: ::std::option::Option::None, x: ::std::option::Option::None, y: ::std::option::Option::None, blur: ::std::option::Option::None }, max_width: ::std::option::Option::None, max_height: ::std::option::Option::None, clip: false, key: format!("{}/@container:232", __ice_use_scope), width: ::std::option::Option::Some(::ui_lang_guest::wire::Length::Fill), height: ::std::option::Option::Some(::ui_lang_guest::wire::Length::Fill), padding: ::std::option::Option::None, align_x: ::std::option::Option::Some(::ui_lang_guest::wire::AlignX::Center), align_y: ::std::option::Option::Some(::ui_lang_guest::wire::AlignY::Center), background: (::std::option::Option::None).map(::ui_lang_guest::wire::Background::Color), border: ::std::option::Option::None, snap: ::std::option::Option::None, content: ::std::boxed::Box::new({
#[cfg(test)]
let __ice_render_source_location = ::ui_lang_runtime::testing::Location::new("src/ui/pages.ice", 238, 1, "rendered view node");
#[cfg(test)]
let __ice_render_source = ::ui_lang_runtime::testing::push_render_source(__ice_render_source_location);
let __ice_rendered: __IceElement<'_, __PagesViewMessage> = ::ui_lang_guest::wire::Node::Text { options: ::ui_lang_guest::wire::TextOptions { height: ::std::option::Option::None, align_y: ::std::option::Option::None, line_height: ::std::option::Option::None, shaping: ::std::option::Option::None, wrapping: ::std::option::Option::None, tracking: 0.0f32, font: ::std::option::Option::Some(::ui_lang_guest::wire::NamedFont { family: ::ui_lang_guest::wire::FontFamily::Named("Geist".into()), weight: ::ui_lang_guest::wire::Weight::Normal, stretch: ::ui_lang_guest::wire::FontStretch::Normal, style: ::ui_lang_guest::wire::FontStyle::Normal }) }, key: format!("{}/@text:238", __ice_use_scope), size: ::std::option::Option::Some(((14.0) as f32).max(f32::EPSILON).min(f32::MAX)), color: ::std::option::Option::None, font: ::ui_lang_guest::wire::Font { monospace: false, weight: ::ui_lang_guest::wire::Weight::Normal }, width: ::std::option::Option::None, align_x: ::std::option::Option::None, content: ("×".to_owned()).to_string() };
#[cfg(test)]
drop(__ice_render_source);
__ice_rendered
}) };
#[cfg(test)]
drop(__ice_render_source);
__ice_rendered
})), label: ::std::option::Option::Some(::std::string::String::from("Clear page search".to_owned())), on_press: if ((!(self.host_error).is_empty())) { ::std::option::Option::None } else { ::std::option::Option::Some(::ui_lang_guest::slots::message((move || __PagesViewMessage::ClearPageSearch)())) }, width: ::std::option::Option::Some(::ui_lang_guest::wire::Length::Fixed((28.0) as f32)), height: ::std::option::Option::Some(::ui_lang_guest::wire::Length::Fixed((28.0) as f32)), padding: ::std::option::Option::Some(::ui_lang_guest::wire::Edges::all((0.0) as f32)), style: ::ui_lang_guest::wire::ButtonStyle { preset: ::ui_lang_guest::wire::ButtonPreset::Primary, recipe: Some(::ui_lang_guest::wire::ButtonRecipe { base: ::ui_lang_guest::wire::Face { background: ::std::option::Option::Some({ let __color: ::iced::Color = ::iced::Color::from_rgba8(0, 0, 0, 0.000000); ::ui_lang_guest::wire::Rgba([__color.r, __color.g, __color.b, __color.a]) }), text: ::std::option::Option::Some({ let __color: ::iced::Color = __ice_palette.colors[4]; ::ui_lang_guest::wire::Rgba([__color.r, __color.g, __color.b, __color.a]) }), border: ::std::option::Option::Some(::ui_lang_guest::wire::Border { color: ::std::option::Option::None, width: ::std::option::Option::None, radius: ::std::option::Option::Some([7.0; 4]) }) }, hover_background: ::std::option::Option::Some({ let __color: ::iced::Color = __ice_palette.colors[14]; ::ui_lang_guest::wire::Rgba([__color.r, __color.g, __color.b, __color.a]) }), pressed_background: ::std::option::Option::Some({ let __color: ::iced::Color = __ice_palette.colors[39]; ::ui_lang_guest::wire::Rgba([__color.r, __color.g, __color.b, __color.a]) }), disabled_background: ::std::option::Option::None, disabled_text: ::std::option::Option::None, disabled_opacity: ::std::option::Option::Some(0.5f32), focus_ring: ::std::option::Option::Some({ let __color: ::iced::Color = __ice_palette.colors[42]; ::ui_lang_guest::wire::Rgba([__color.r, __color.g, __color.b, __color.a]) }), text_size: ::std::option::Option::Some(13.5f32), line_height: ::std::option::Option::None, font: Some(::ui_lang_guest::wire::NamedFont { family: ::ui_lang_guest::wire::FontFamily::Named("Geist".into()), weight: ::ui_lang_guest::wire::Weight::Normal, stretch: ::ui_lang_guest::wire::FontStretch::Normal, style: ::ui_lang_guest::wire::FontStyle::Normal }) }), active: ::ui_lang_guest::wire::Face { background: ::std::option::Option::Some({ let __color: ::iced::Color = ::iced::Color::from_rgba8(0, 0, 0, 0.000000); ::ui_lang_guest::wire::Rgba([__color.r, __color.g, __color.b, __color.a]) }), text: ::std::option::Option::Some({ let __color: ::iced::Color = __ice_palette.colors[5]; ::ui_lang_guest::wire::Rgba([__color.r, __color.g, __color.b, __color.a]) }), border: ::std::option::Option::Some(::ui_lang_guest::wire::Border { color: ::std::option::Option::None, width: ::std::option::Option::None, radius: ::std::option::Option::Some([((7.0) as f32).max(0.0).min(f32::MAX), ((7.0) as f32).max(0.0).min(f32::MAX), ((7.0) as f32).max(0.0).min(f32::MAX), ((7.0) as f32).max(0.0).min(f32::MAX)]) }) }, hovered: ::std::option::Option::Some(::ui_lang_guest::wire::Face { background: ::std::option::Option::Some({ let __color: ::iced::Color = { let mut __color = __ice_palette.colors[4]; __color.a = 0.100000; __color }; ::ui_lang_guest::wire::Rgba([__color.r, __color.g, __color.b, __color.a]) }), text: ::std::option::Option::Some({ let __color: ::iced::Color = __ice_palette.colors[4]; ::ui_lang_guest::wire::Rgba([__color.r, __color.g, __color.b, __color.a]) }), border: ::std::option::Option::None }), pressed: ::std::option::Option::Some(::ui_lang_guest::wire::Face { background: ::std::option::Option::Some({ let __color: ::iced::Color = { let mut __color = __ice_palette.colors[4]; __color.a = 0.150000; __color }; ::ui_lang_guest::wire::Rgba([__color.r, __color.g, __color.b, __color.a]) }), text: ::std::option::Option::None, border: ::std::option::Option::None }), disabled: ::std::option::Option::None } };
#[cfg(test)]
drop(__ice_render_source);
__ice_rendered
}); } __children.push({
#[cfg(test)]
let __ice_render_source_location = ::ui_lang_runtime::testing::Location::new("src/ui/pages.ice", 249, 1, "rendered view node");
#[cfg(test)]
let __ice_render_source = ::ui_lang_runtime::testing::push_render_source(__ice_render_source_location);
let __ice_rendered: __IceElement<'_, __PagesViewMessage> = ::ui_lang_guest::wire::Node::Button { checked: ::std::option::Option::None, expanded: ::std::option::Option::Some(self.block_comments_open), description: ::std::option::Option::None, key: format!("{}/@button:249", __ice_use_scope), content: ::ui_lang_guest::wire::ButtonContent::Child(::std::boxed::Box::new({
#[cfg(test)]
let __ice_render_source_location = ::ui_lang_runtime::testing::Location::new("src/ui/pages.ice", 257, 1, "rendered view node");
#[cfg(test)]
let __ice_render_source = ::ui_lang_runtime::testing::push_render_source(__ice_render_source_location);
let __ice_rendered: __IceElement<'_, __PagesViewMessage> = { let mut __children: ::std::vec::Vec<__IceElement<'_, __PagesViewMessage>> = ::std::vec::Vec::new(); __children.push({
#[cfg(test)]
let __ice_render_source_location = ::ui_lang_runtime::testing::Location::new("src/ui/pages.ice", 262, 1, "rendered view node");
#[cfg(test)]
let __ice_render_source = ::ui_lang_runtime::testing::push_render_source(__ice_render_source_location);
let __ice_rendered: __IceElement<'_, __PagesViewMessage> = ::ui_lang_runtime::grow_stack(|| self.__ice_component_use_4(__ice_palette, format!("{}/Icon@883", __ice_use_scope)));
#[cfg(test)]
drop(__ice_render_source);
__ice_rendered
}); __children.push({
#[cfg(test)]
let __ice_render_source_location = ::ui_lang_runtime::testing::Location::new("src/ui/pages.ice", 267, 1, "rendered view node");
#[cfg(test)]
let __ice_render_source = ::ui_lang_runtime::testing::push_render_source(__ice_render_source_location);
let __ice_rendered: __IceElement<'_, __PagesViewMessage> = ::ui_lang_guest::wire::Node::Text { options: ::ui_lang_guest::wire::TextOptions { height: ::std::option::Option::None, align_y: ::std::option::Option::None, line_height: ::std::option::Option::None, shaping: ::std::option::Option::None, wrapping: ::std::option::Option::Some(::ui_lang_guest::wire::Wrapping::None), tracking: 0.0f32, font: ::std::option::Option::Some(::ui_lang_guest::wire::NamedFont { family: ::ui_lang_guest::wire::FontFamily::Named("Geist".into()), weight: ::ui_lang_guest::wire::Weight::Normal, stretch: ::ui_lang_guest::wire::FontStretch::Normal, style: ::ui_lang_guest::wire::FontStyle::Normal }) }, key: format!("{}/@text:267", __ice_use_scope), size: ::std::option::Option::Some(((11.5) as f32).max(f32::EPSILON).min(f32::MAX)), color: ::std::option::Option::Some({ let __color: ::iced::Color = __ice_palette.colors[5]; ::ui_lang_guest::wire::Rgba([__color.r, __color.g, __color.b, __color.a]) }), font: ::ui_lang_guest::wire::Font { monospace: false, weight: ::ui_lang_guest::wire::Weight::Normal }, width: ::std::option::Option::None, align_x: ::std::option::Option::None, content: ("Comments".to_owned()).to_string() };
#[cfg(test)]
drop(__ice_render_source);
__ice_rendered
}); if (self.thread_total > 0) { __children.push({
#[cfg(test)]
let __ice_render_source_location = ::ui_lang_runtime::testing::Location::new("src/ui/pages.ice", 273, 1, "rendered view node");
#[cfg(test)]
let __ice_render_source = ::ui_lang_runtime::testing::push_render_source(__ice_render_source_location);
let __ice_rendered: __IceElement<'_, __PagesViewMessage> = ::ui_lang_guest::wire::Node::Text { options: ::ui_lang_guest::wire::TextOptions { height: ::std::option::Option::None, align_y: ::std::option::Option::None, line_height: ::std::option::Option::None, shaping: ::std::option::Option::None, wrapping: ::std::option::Option::Some(::ui_lang_guest::wire::Wrapping::None), tracking: 0.0f32, font: ::std::option::Option::Some(::ui_lang_guest::wire::NamedFont { family: ::ui_lang_guest::wire::FontFamily::Named("Geist Mono".into()), weight: ::ui_lang_guest::wire::Weight::Medium, stretch: ::ui_lang_guest::wire::FontStretch::Normal, style: ::ui_lang_guest::wire::FontStyle::Normal }) }, key: format!("{}/@text:273", __ice_use_scope), size: ::std::option::Option::Some(((10.5) as f32).max(f32::EPSILON).min(f32::MAX)), color: ::std::option::Option::Some({ let __color: ::iced::Color = __ice_palette.colors[73]; ::ui_lang_guest::wire::Rgba([__color.r, __color.g, __color.b, __color.a]) }), font: ::ui_lang_guest::wire::Font { monospace: false, weight: ::ui_lang_guest::wire::Weight::Normal }, width: ::std::option::Option::None, align_x: ::std::option::Option::None, content: (crate::host::count_label(self.thread_total)).to_string() };
#[cfg(test)]
drop(__ice_render_source);
__ice_rendered
}); } ::ui_lang_guest::wire::Node::Linear { max_width: ::std::option::Option::None, clip: false, key: format!("{}/@layout:257", __ice_use_scope), wrap: None, axis: ::ui_lang_guest::wire::Axis::Row, spacing: ::std::option::Option::Some((5.0) as f32), padding: ::std::option::Option::None, width: ::std::option::Option::None, height: ::std::option::Option::Some(::ui_lang_guest::wire::Length::Fill), align: ::std::option::Option::Some(::ui_lang_guest::wire::AlignX::Center), background: ::std::option::Option::None, border: ::std::option::Option::None, children: __children } };
#[cfg(test)]
drop(__ice_render_source);
__ice_rendered
})), label: ::std::option::Option::Some(::std::string::String::from("Comments".to_owned())), on_press: if ((self.busy || (!(self.host_error).is_empty()))) { ::std::option::Option::None } else { ::std::option::Option::Some(::ui_lang_guest::slots::message((move || __PagesViewMessage::ToggleBlockComments)())) }, width: ::std::option::Option::None, height: ::std::option::Option::Some(::ui_lang_guest::wire::Length::Fixed((26.0) as f32)), padding: ::std::option::Option::Some(::ui_lang_guest::wire::Edges::all((5.0) as f32)), style: ::ui_lang_guest::wire::ButtonStyle { preset: ::ui_lang_guest::wire::ButtonPreset::Primary, recipe: Some(::ui_lang_guest::wire::ButtonRecipe { base: ::ui_lang_guest::wire::Face { background: ::std::option::Option::Some({ let __color: ::iced::Color = ::iced::Color::from_rgba8(0, 0, 0, 0.000000); ::ui_lang_guest::wire::Rgba([__color.r, __color.g, __color.b, __color.a]) }), text: ::std::option::Option::Some({ let __color: ::iced::Color = __ice_palette.colors[4]; ::ui_lang_guest::wire::Rgba([__color.r, __color.g, __color.b, __color.a]) }), border: ::std::option::Option::Some(::ui_lang_guest::wire::Border { color: ::std::option::Option::None, width: ::std::option::Option::None, radius: ::std::option::Option::Some([8.0; 4]) }) }, hover_background: ::std::option::Option::Some({ let __color: ::iced::Color = __ice_palette.colors[14]; ::ui_lang_guest::wire::Rgba([__color.r, __color.g, __color.b, __color.a]) }), pressed_background: ::std::option::Option::Some({ let __color: ::iced::Color = __ice_palette.colors[39]; ::ui_lang_guest::wire::Rgba([__color.r, __color.g, __color.b, __color.a]) }), disabled_background: ::std::option::Option::None, disabled_text: ::std::option::Option::None, disabled_opacity: ::std::option::Option::Some(0.5f32), focus_ring: ::std::option::Option::Some({ let __color: ::iced::Color = __ice_palette.colors[42]; ::ui_lang_guest::wire::Rgba([__color.r, __color.g, __color.b, __color.a]) }), text_size: ::std::option::Option::Some(12.5f32), line_height: ::std::option::Option::None, font: Some(::ui_lang_guest::wire::NamedFont { family: ::ui_lang_guest::wire::FontFamily::Named("Geist".into()), weight: ::ui_lang_guest::wire::Weight::Semibold, stretch: ::ui_lang_guest::wire::FontStretch::Normal, style: ::ui_lang_guest::wire::FontStyle::Normal }) }), active: ::ui_lang_guest::wire::Face { background: ::std::option::Option::Some({ let __color: ::iced::Color = ::iced::Color::from_rgba8(0, 0, 0, 0.000000); ::ui_lang_guest::wire::Rgba([__color.r, __color.g, __color.b, __color.a]) }), text: ::std::option::Option::Some({ let __color: ::iced::Color = __ice_palette.colors[5]; ::ui_lang_guest::wire::Rgba([__color.r, __color.g, __color.b, __color.a]) }), border: ::std::option::Option::Some(::ui_lang_guest::wire::Border { color: ::std::option::Option::Some({ let __color: ::iced::Color = ::iced::Color::from_rgba8(0, 0, 0, 0.000000); ::ui_lang_guest::wire::Rgba([__color.r, __color.g, __color.b, __color.a]) }), width: ::std::option::Option::Some(((1.0) as f32).max(0.0).min(f32::MAX)), radius: ::std::option::Option::Some([((7.0) as f32).max(0.0).min(f32::MAX), ((7.0) as f32).max(0.0).min(f32::MAX), ((7.0) as f32).max(0.0).min(f32::MAX), ((7.0) as f32).max(0.0).min(f32::MAX)]) }) }, hovered: ::std::option::Option::Some(::ui_lang_guest::wire::Face { background: ::std::option::Option::Some({ let __color: ::iced::Color = { let mut __color = __ice_palette.colors[4]; __color.a = 0.080000; __color }; ::ui_lang_guest::wire::Rgba([__color.r, __color.g, __color.b, __color.a]) }), text: ::std::option::Option::Some({ let __color: ::iced::Color = __ice_palette.colors[4]; ::ui_lang_guest::wire::Rgba([__color.r, __color.g, __color.b, __color.a]) }), border: ::std::option::Option::Some(::ui_lang_guest::wire::Border { color: ::std::option::Option::Some({ let __color: ::iced::Color = { let mut __color = __ice_palette.colors[4]; __color.a = 0.100000; __color }; ::ui_lang_guest::wire::Rgba([__color.r, __color.g, __color.b, __color.a]) }), width: ::std::option::Option::None, radius: ::std::option::Option::None }) }), pressed: ::std::option::Option::Some(::ui_lang_guest::wire::Face { background: ::std::option::Option::Some({ let __color: ::iced::Color = { let mut __color = __ice_palette.colors[4]; __color.a = 0.120000; __color }; ::ui_lang_guest::wire::Rgba([__color.r, __color.g, __color.b, __color.a]) }), text: ::std::option::Option::Some({ let __color: ::iced::Color = __ice_palette.colors[4]; ::ui_lang_guest::wire::Rgba([__color.r, __color.g, __color.b, __color.a]) }), border: ::std::option::Option::None }), disabled: ::std::option::Option::None } };
#[cfg(test)]
drop(__ice_render_source);
__ice_rendered
}); __children.push({
#[cfg(test)]
let __ice_render_source_location = ::ui_lang_runtime::testing::Location::new("src/ui/pages.ice", 285, 1, "rendered view node");
#[cfg(test)]
let __ice_render_source = ::ui_lang_runtime::testing::push_render_source(__ice_render_source_location);
let __ice_rendered: __IceElement<'_, __PagesViewMessage> = ::ui_lang_guest::wire::Node::Button { checked: ::std::option::Option::None, expanded: ::std::option::Option::None, description: ::std::option::Option::None, key: format!("{}/@button:285", __ice_use_scope), content: ::ui_lang_guest::wire::ButtonContent::Child(::std::boxed::Box::new({
#[cfg(test)]
let __ice_render_source_location = ::ui_lang_runtime::testing::Location::new("src/ui/pages.ice", 293, 1, "rendered view node");
#[cfg(test)]
let __ice_render_source = ::ui_lang_runtime::testing::push_render_source(__ice_render_source_location);
let __ice_rendered: __IceElement<'_, __PagesViewMessage> = ::ui_lang_guest::wire::Node::Container { shadow: ::ui_lang_guest::wire::Shadow { color: ::std::option::Option::None, x: ::std::option::Option::None, y: ::std::option::Option::None, blur: ::std::option::Option::None }, max_width: ::std::option::Option::None, max_height: ::std::option::Option::None, clip: false, key: format!("{}/@container:293", __ice_use_scope), width: ::std::option::Option::Some(::ui_lang_guest::wire::Length::Fill), height: ::std::option::Option::Some(::ui_lang_guest::wire::Length::Fill), padding: ::std::option::Option::None, align_x: ::std::option::Option::Some(::ui_lang_guest::wire::AlignX::Center), align_y: ::std::option::Option::Some(::ui_lang_guest::wire::AlignY::Center), background: (::std::option::Option::None).map(::ui_lang_guest::wire::Background::Color), border: ::std::option::Option::None, snap: ::std::option::Option::None, content: ::std::boxed::Box::new({
#[cfg(test)]
let __ice_render_source_location = ::ui_lang_runtime::testing::Location::new("src/ui/pages.ice", 299, 1, "rendered view node");
#[cfg(test)]
let __ice_render_source = ::ui_lang_runtime::testing::push_render_source(__ice_render_source_location);
let __ice_rendered: __IceElement<'_, __PagesViewMessage> = ::ui_lang_runtime::grow_stack(|| self.__ice_component_use_5(__ice_palette, format!("{}/Icon@920", __ice_use_scope)));
#[cfg(test)]
drop(__ice_render_source);
__ice_rendered
}) };
#[cfg(test)]
drop(__ice_render_source);
__ice_rendered
})), label: ::std::option::Option::Some(::std::string::String::from("Copy page link".to_owned())), on_press: if (((!(self.host_error).is_empty()) || (self.active_page).is_empty())) { ::std::option::Option::None } else { ::std::option::Option::Some(::ui_lang_guest::slots::message((move |__event_0, __event_1| __PagesViewMessage::CopyToClipboard(__event_0, __event_1))(self.page_link.to_owned(), "Page link copied".to_owned()))) }, width: ::std::option::Option::Some(::ui_lang_guest::wire::Length::Fixed((28.0) as f32)), height: ::std::option::Option::Some(::ui_lang_guest::wire::Length::Fixed((28.0) as f32)), padding: ::std::option::Option::Some(::ui_lang_guest::wire::Edges::all((0.0) as f32)), style: ::ui_lang_guest::wire::ButtonStyle { preset: ::ui_lang_guest::wire::ButtonPreset::Primary, recipe: Some(::ui_lang_guest::wire::ButtonRecipe { base: ::ui_lang_guest::wire::Face { background: ::std::option::Option::Some({ let __color: ::iced::Color = ::iced::Color::from_rgba8(0, 0, 0, 0.000000); ::ui_lang_guest::wire::Rgba([__color.r, __color.g, __color.b, __color.a]) }), text: ::std::option::Option::Some({ let __color: ::iced::Color = __ice_palette.colors[4]; ::ui_lang_guest::wire::Rgba([__color.r, __color.g, __color.b, __color.a]) }), border: ::std::option::Option::Some(::ui_lang_guest::wire::Border { color: ::std::option::Option::None, width: ::std::option::Option::None, radius: ::std::option::Option::Some([7.0; 4]) }) }, hover_background: ::std::option::Option::Some({ let __color: ::iced::Color = __ice_palette.colors[14]; ::ui_lang_guest::wire::Rgba([__color.r, __color.g, __color.b, __color.a]) }), pressed_background: ::std::option::Option::Some({ let __color: ::iced::Color = __ice_palette.colors[39]; ::ui_lang_guest::wire::Rgba([__color.r, __color.g, __color.b, __color.a]) }), disabled_background: ::std::option::Option::None, disabled_text: ::std::option::Option::None, disabled_opacity: ::std::option::Option::Some(0.5f32), focus_ring: ::std::option::Option::Some({ let __color: ::iced::Color = __ice_palette.colors[42]; ::ui_lang_guest::wire::Rgba([__color.r, __color.g, __color.b, __color.a]) }), text_size: ::std::option::Option::Some(13.5f32), line_height: ::std::option::Option::None, font: Some(::ui_lang_guest::wire::NamedFont { family: ::ui_lang_guest::wire::FontFamily::Named("Geist".into()), weight: ::ui_lang_guest::wire::Weight::Normal, stretch: ::ui_lang_guest::wire::FontStretch::Normal, style: ::ui_lang_guest::wire::FontStyle::Normal }) }), active: ::ui_lang_guest::wire::Face { background: ::std::option::Option::Some({ let __color: ::iced::Color = ::iced::Color::from_rgba8(0, 0, 0, 0.000000); ::ui_lang_guest::wire::Rgba([__color.r, __color.g, __color.b, __color.a]) }), text: ::std::option::Option::Some({ let __color: ::iced::Color = __ice_palette.colors[5]; ::ui_lang_guest::wire::Rgba([__color.r, __color.g, __color.b, __color.a]) }), border: ::std::option::Option::Some(::ui_lang_guest::wire::Border { color: ::std::option::Option::None, width: ::std::option::Option::None, radius: ::std::option::Option::Some([((7.0) as f32).max(0.0).min(f32::MAX), ((7.0) as f32).max(0.0).min(f32::MAX), ((7.0) as f32).max(0.0).min(f32::MAX), ((7.0) as f32).max(0.0).min(f32::MAX)]) }) }, hovered: ::std::option::Option::Some(::ui_lang_guest::wire::Face { background: ::std::option::Option::Some({ let __color: ::iced::Color = { let mut __color = __ice_palette.colors[4]; __color.a = 0.100000; __color }; ::ui_lang_guest::wire::Rgba([__color.r, __color.g, __color.b, __color.a]) }), text: ::std::option::Option::Some({ let __color: ::iced::Color = __ice_palette.colors[4]; ::ui_lang_guest::wire::Rgba([__color.r, __color.g, __color.b, __color.a]) }), border: ::std::option::Option::None }), pressed: ::std::option::Option::Some(::ui_lang_guest::wire::Face { background: ::std::option::Option::Some({ let __color: ::iced::Color = { let mut __color = __ice_palette.colors[4]; __color.a = 0.150000; __color }; ::ui_lang_guest::wire::Rgba([__color.r, __color.g, __color.b, __color.a]) }), text: ::std::option::Option::None, border: ::std::option::Option::None }), disabled: ::std::option::Option::None } };
#[cfg(test)]
drop(__ice_render_source);
__ice_rendered
}); __children.push({
#[cfg(test)]
let __ice_render_source_location = ::ui_lang_runtime::testing::Location::new("src/ui/pages.ice", 312, 1, "rendered view node");
#[cfg(test)]
let __ice_render_source = ::ui_lang_runtime::testing::push_render_source(__ice_render_source_location);
let __ice_rendered: __IceElement<'_, __PagesViewMessage> = { let __ice_node_scope = format!("{}/page-menu-trigger", __ice_use_scope); ::ui_lang_guest::wire::Node::Button { checked: ::std::option::Option::None, expanded: ::std::option::Option::Some(self.page_menu_open), description: ::std::option::Option::None, key: __ice_node_scope.clone(), content: ::ui_lang_guest::wire::ButtonContent::Child(::std::boxed::Box::new({
#[cfg(test)]
let __ice_render_source_location = ::ui_lang_runtime::testing::Location::new("src/ui/pages.ice", 321, 1, "rendered view node");
#[cfg(test)]
let __ice_render_source = ::ui_lang_runtime::testing::push_render_source(__ice_render_source_location);
let __ice_rendered: __IceElement<'_, __PagesViewMessage> = ::ui_lang_guest::wire::Node::Container { shadow: ::ui_lang_guest::wire::Shadow { color: ::std::option::Option::None, x: ::std::option::Option::None, y: ::std::option::Option::None, blur: ::std::option::Option::None }, max_width: ::std::option::Option::None, max_height: ::std::option::Option::None, clip: false, key: format!("{}/@container:321", __ice_use_scope), width: ::std::option::Option::Some(::ui_lang_guest::wire::Length::Fill), height: ::std::option::Option::Some(::ui_lang_guest::wire::Length::Fill), padding: ::std::option::Option::None, align_x: ::std::option::Option::Some(::ui_lang_guest::wire::AlignX::Center), align_y: ::std::option::Option::Some(::ui_lang_guest::wire::AlignY::Center), background: (::std::option::Option::None).map(::ui_lang_guest::wire::Background::Color), border: ::std::option::Option::None, snap: ::std::option::Option::None, content: ::std::boxed::Box::new({
#[cfg(test)]
let __ice_render_source_location = ::ui_lang_runtime::testing::Location::new("src/ui/pages.ice", 327, 1, "rendered view node");
#[cfg(test)]
let __ice_render_source = ::ui_lang_runtime::testing::push_render_source(__ice_render_source_location);
let __ice_rendered: __IceElement<'_, __PagesViewMessage> = ::ui_lang_guest::wire::Node::Text { options: ::ui_lang_guest::wire::TextOptions { height: ::std::option::Option::None, align_y: ::std::option::Option::None, line_height: ::std::option::Option::None, shaping: ::std::option::Option::None, wrapping: ::std::option::Option::None, tracking: 0.0f32, font: ::std::option::Option::Some(::ui_lang_guest::wire::NamedFont { family: ::ui_lang_guest::wire::FontFamily::Named("Geist".into()), weight: ::ui_lang_guest::wire::Weight::Normal, stretch: ::ui_lang_guest::wire::FontStretch::Normal, style: ::ui_lang_guest::wire::FontStyle::Normal }) }, key: format!("{}/@text:327", __ice_use_scope), size: ::std::option::Option::Some(((13.0) as f32).max(f32::EPSILON).min(f32::MAX)), color: ::std::option::Option::None, font: ::ui_lang_guest::wire::Font { monospace: false, weight: ::ui_lang_guest::wire::Weight::Normal }, width: ::std::option::Option::None, align_x: ::std::option::Option::None, content: ("•••".to_owned()).to_string() };
#[cfg(test)]
drop(__ice_render_source);
__ice_rendered
}) };
#[cfg(test)]
drop(__ice_render_source);
__ice_rendered
})), label: ::std::option::Option::Some(::std::string::String::from("Page actions".to_owned())), on_press: if (((self.busy || (!(self.host_error).is_empty())) || self.page_delete_armed)) { ::std::option::Option::None } else { ::std::option::Option::Some(::ui_lang_guest::slots::message((move || __PagesViewMessage::TogglePageMenu)())) }, width: ::std::option::Option::Some(::ui_lang_guest::wire::Length::Fixed((28.0) as f32)), height: ::std::option::Option::Some(::ui_lang_guest::wire::Length::Fixed((28.0) as f32)), padding: ::std::option::Option::Some(::ui_lang_guest::wire::Edges::all((0.0) as f32)), style: ::ui_lang_guest::wire::ButtonStyle { preset: ::ui_lang_guest::wire::ButtonPreset::Primary, recipe: Some(::ui_lang_guest::wire::ButtonRecipe { base: ::ui_lang_guest::wire::Face { background: ::std::option::Option::Some({ let __color: ::iced::Color = ::iced::Color::from_rgba8(0, 0, 0, 0.000000); ::ui_lang_guest::wire::Rgba([__color.r, __color.g, __color.b, __color.a]) }), text: ::std::option::Option::Some({ let __color: ::iced::Color = __ice_palette.colors[4]; ::ui_lang_guest::wire::Rgba([__color.r, __color.g, __color.b, __color.a]) }), border: ::std::option::Option::Some(::ui_lang_guest::wire::Border { color: ::std::option::Option::None, width: ::std::option::Option::None, radius: ::std::option::Option::Some([7.0; 4]) }) }, hover_background: ::std::option::Option::Some({ let __color: ::iced::Color = __ice_palette.colors[14]; ::ui_lang_guest::wire::Rgba([__color.r, __color.g, __color.b, __color.a]) }), pressed_background: ::std::option::Option::Some({ let __color: ::iced::Color = __ice_palette.colors[39]; ::ui_lang_guest::wire::Rgba([__color.r, __color.g, __color.b, __color.a]) }), disabled_background: ::std::option::Option::None, disabled_text: ::std::option::Option::None, disabled_opacity: ::std::option::Option::Some(0.5f32), focus_ring: ::std::option::Option::Some({ let __color: ::iced::Color = __ice_palette.colors[42]; ::ui_lang_guest::wire::Rgba([__color.r, __color.g, __color.b, __color.a]) }), text_size: ::std::option::Option::Some(13.5f32), line_height: ::std::option::Option::None, font: Some(::ui_lang_guest::wire::NamedFont { family: ::ui_lang_guest::wire::FontFamily::Named("Geist".into()), weight: ::ui_lang_guest::wire::Weight::Normal, stretch: ::ui_lang_guest::wire::FontStretch::Normal, style: ::ui_lang_guest::wire::FontStyle::Normal }) }), active: ::ui_lang_guest::wire::Face { background: ::std::option::Option::Some({ let __color: ::iced::Color = ::iced::Color::from_rgba8(0, 0, 0, 0.000000); ::ui_lang_guest::wire::Rgba([__color.r, __color.g, __color.b, __color.a]) }), text: ::std::option::Option::Some({ let __color: ::iced::Color = __ice_palette.colors[5]; ::ui_lang_guest::wire::Rgba([__color.r, __color.g, __color.b, __color.a]) }), border: ::std::option::Option::Some(::ui_lang_guest::wire::Border { color: ::std::option::Option::None, width: ::std::option::Option::None, radius: ::std::option::Option::Some([((7.0) as f32).max(0.0).min(f32::MAX), ((7.0) as f32).max(0.0).min(f32::MAX), ((7.0) as f32).max(0.0).min(f32::MAX), ((7.0) as f32).max(0.0).min(f32::MAX)]) }) }, hovered: ::std::option::Option::Some(::ui_lang_guest::wire::Face { background: ::std::option::Option::Some({ let __color: ::iced::Color = { let mut __color = __ice_palette.colors[4]; __color.a = 0.100000; __color }; ::ui_lang_guest::wire::Rgba([__color.r, __color.g, __color.b, __color.a]) }), text: ::std::option::Option::Some({ let __color: ::iced::Color = __ice_palette.colors[4]; ::ui_lang_guest::wire::Rgba([__color.r, __color.g, __color.b, __color.a]) }), border: ::std::option::Option::None }), pressed: ::std::option::Option::Some(::ui_lang_guest::wire::Face { background: ::std::option::Option::Some({ let __color: ::iced::Color = { let mut __color = __ice_palette.colors[4]; __color.a = 0.150000; __color }; ::ui_lang_guest::wire::Rgba([__color.r, __color.g, __color.b, __color.a]) }), text: ::std::option::Option::None, border: ::std::option::Option::None }), disabled: ::std::option::Option::None } } };
#[cfg(test)]
drop(__ice_render_source);
__ice_rendered
}); if (self.autosave == "saving") { __children.push({
#[cfg(test)]
let __ice_render_source_location = ::ui_lang_runtime::testing::Location::new("src/ui/pages.ice", 337, 1, "rendered view node");
#[cfg(test)]
let __ice_render_source = ::ui_lang_runtime::testing::push_render_source(__ice_render_source_location);
let __ice_rendered: __IceElement<'_, __PagesViewMessage> = ::ui_lang_guest::wire::Node::Container { shadow: ::ui_lang_guest::wire::Shadow { color: ::std::option::Option::None, x: ::std::option::Option::None, y: ::std::option::Option::None, blur: ::std::option::Option::None }, max_width: ::std::option::Option::None, max_height: ::std::option::Option::None, clip: false, key: format!("{}/@container:337", __ice_use_scope), width: ::std::option::Option::None, height: ::std::option::Option::None, padding: ::std::option::Option::Some(::ui_lang_guest::wire::Edges { top: (4.0) as f32, right: (9.0) as f32, bottom: (4.0) as f32, left: (9.0) as f32 }), align_x: ::std::option::Option::None, align_y: ::std::option::Option::None, background: (::std::option::Option::Some({ let __color: ::iced::Color = __ice_palette.colors[32]; ::ui_lang_guest::wire::Rgba([__color.r, __color.g, __color.b, __color.a]) })).map(::ui_lang_guest::wire::Background::Color), border: ::std::option::Option::Some(::ui_lang_guest::wire::Border { color: ::std::option::Option::Some({ let __color: ::iced::Color = __ice_palette.colors[33]; ::ui_lang_guest::wire::Rgba([__color.r, __color.g, __color.b, __color.a]) }), width: ::std::option::Option::Some(((1.0) as f32).max(0.0).min(f32::MAX)), radius: ::std::option::Option::Some([((7.0) as f32).max(0.0).min(f32::MAX), ((7.0) as f32).max(0.0).min(f32::MAX), ((7.0) as f32).max(0.0).min(f32::MAX), ((7.0) as f32).max(0.0).min(f32::MAX)]) }), snap: ::std::option::Option::None, content: ::std::boxed::Box::new({
#[cfg(test)]
let __ice_render_source_location = ::ui_lang_runtime::testing::Location::new("src/ui/pages.ice", 345, 1, "rendered view node");
#[cfg(test)]
let __ice_render_source = ::ui_lang_runtime::testing::push_render_source(__ice_render_source_location);
let __ice_rendered: __IceElement<'_, __PagesViewMessage> = ::ui_lang_guest::wire::Node::Text { options: ::ui_lang_guest::wire::TextOptions { height: ::std::option::Option::None, align_y: ::std::option::Option::None, line_height: ::std::option::Option::None, shaping: ::std::option::Option::None, wrapping: ::std::option::Option::Some(::ui_lang_guest::wire::Wrapping::None), tracking: 0.0f32, font: ::std::option::Option::Some(::ui_lang_guest::wire::NamedFont { family: ::ui_lang_guest::wire::FontFamily::Named("Geist Mono".into()), weight: ::ui_lang_guest::wire::Weight::Medium, stretch: ::ui_lang_guest::wire::FontStretch::Normal, style: ::ui_lang_guest::wire::FontStyle::Normal }) }, key: format!("{}/@text:345", __ice_use_scope), size: ::std::option::Option::Some(((10.5) as f32).max(f32::EPSILON).min(f32::MAX)), color: ::std::option::Option::Some({ let __color: ::iced::Color = __ice_palette.colors[30]; ::ui_lang_guest::wire::Rgba([__color.r, __color.g, __color.b, __color.a]) }), font: ::ui_lang_guest::wire::Font { monospace: false, weight: ::ui_lang_guest::wire::Weight::Normal }, width: ::std::option::Option::None, align_x: ::std::option::Option::None, content: ("saving…".to_owned()).to_string() };
#[cfg(test)]
drop(__ice_render_source);
__ice_rendered
}) };
#[cfg(test)]
drop(__ice_render_source);
__ice_rendered
}); } if (self.autosave == "error") { __children.push({
#[cfg(test)]
let __ice_render_source_location = ::ui_lang_runtime::testing::Location::new("src/ui/pages.ice", 352, 1, "rendered view node");
#[cfg(test)]
let __ice_render_source = ::ui_lang_runtime::testing::push_render_source(__ice_render_source_location);
let __ice_rendered: __IceElement<'_, __PagesViewMessage> = ::ui_lang_guest::wire::Node::Container { shadow: ::ui_lang_guest::wire::Shadow { color: ::std::option::Option::None, x: ::std::option::Option::None, y: ::std::option::Option::None, blur: ::std::option::Option::None }, max_width: ::std::option::Option::None, max_height: ::std::option::Option::None, clip: false, key: format!("{}/@container:352", __ice_use_scope), width: ::std::option::Option::None, height: ::std::option::Option::None, padding: ::std::option::Option::Some(::ui_lang_guest::wire::Edges { top: (4.0) as f32, right: (9.0) as f32, bottom: (4.0) as f32, left: (9.0) as f32 }), align_x: ::std::option::Option::None, align_y: ::std::option::Option::None, background: (::std::option::Option::Some({ let __color: ::iced::Color = __ice_palette.colors[22]; ::ui_lang_guest::wire::Rgba([__color.r, __color.g, __color.b, __color.a]) })).map(::ui_lang_guest::wire::Background::Color), border: ::std::option::Option::Some(::ui_lang_guest::wire::Border { color: ::std::option::Option::Some({ let __color: ::iced::Color = __ice_palette.colors[23]; ::ui_lang_guest::wire::Rgba([__color.r, __color.g, __color.b, __color.a]) }), width: ::std::option::Option::Some(((1.0) as f32).max(0.0).min(f32::MAX)), radius: ::std::option::Option::Some([((7.0) as f32).max(0.0).min(f32::MAX), ((7.0) as f32).max(0.0).min(f32::MAX), ((7.0) as f32).max(0.0).min(f32::MAX), ((7.0) as f32).max(0.0).min(f32::MAX)]) }), snap: ::std::option::Option::None, content: ::std::boxed::Box::new({
#[cfg(test)]
let __ice_render_source_location = ::ui_lang_runtime::testing::Location::new("src/ui/pages.ice", 360, 1, "rendered view node");
#[cfg(test)]
let __ice_render_source = ::ui_lang_runtime::testing::push_render_source(__ice_render_source_location);
let __ice_rendered: __IceElement<'_, __PagesViewMessage> = ::ui_lang_guest::wire::Node::Text { options: ::ui_lang_guest::wire::TextOptions { height: ::std::option::Option::None, align_y: ::std::option::Option::None, line_height: ::std::option::Option::None, shaping: ::std::option::Option::None, wrapping: ::std::option::Option::Some(::ui_lang_guest::wire::Wrapping::None), tracking: 0.0f32, font: ::std::option::Option::Some(::ui_lang_guest::wire::NamedFont { family: ::ui_lang_guest::wire::FontFamily::Named("Geist Mono".into()), weight: ::ui_lang_guest::wire::Weight::Medium, stretch: ::ui_lang_guest::wire::FontStretch::Normal, style: ::ui_lang_guest::wire::FontStyle::Normal }) }, key: format!("{}/@text:360", __ice_use_scope), size: ::std::option::Option::Some(((10.5) as f32).max(f32::EPSILON).min(f32::MAX)), color: ::std::option::Option::Some({ let __color: ::iced::Color = __ice_palette.colors[20]; ::ui_lang_guest::wire::Rgba([__color.r, __color.g, __color.b, __color.a]) }), font: ::ui_lang_guest::wire::Font { monospace: false, weight: ::ui_lang_guest::wire::Weight::Normal }, width: ::std::option::Option::None, align_x: ::std::option::Option::None, content: ("not saved".to_owned()).to_string() };
#[cfg(test)]
drop(__ice_render_source);
__ice_rendered
}) };
#[cfg(test)]
drop(__ice_render_source);
__ice_rendered
}); } if (self.autosave == "saved") { __children.push({
#[cfg(test)]
let __ice_render_source_location = ::ui_lang_runtime::testing::Location::new("src/ui/pages.ice", 367, 1, "rendered view node");
#[cfg(test)]
let __ice_render_source = ::ui_lang_runtime::testing::push_render_source(__ice_render_source_location);
let __ice_rendered: __IceElement<'_, __PagesViewMessage> = ::ui_lang_guest::wire::Node::Container { shadow: ::ui_lang_guest::wire::Shadow { color: ::std::option::Option::None, x: ::std::option::Option::None, y: ::std::option::Option::None, blur: ::std::option::Option::None }, max_width: ::std::option::Option::None, max_height: ::std::option::Option::None, clip: false, key: format!("{}/@container:367", __ice_use_scope), width: ::std::option::Option::None, height: ::std::option::Option::None, padding: ::std::option::Option::Some(::ui_lang_guest::wire::Edges { top: (4.0) as f32, right: (9.0) as f32, bottom: (4.0) as f32, left: (9.0) as f32 }), align_x: ::std::option::Option::None, align_y: ::std::option::Option::None, background: (::std::option::Option::Some({ let __color: ::iced::Color = __ice_palette.colors[106]; ::ui_lang_guest::wire::Rgba([__color.r, __color.g, __color.b, __color.a]) })).map(::ui_lang_guest::wire::Background::Color), border: ::std::option::Option::Some(::ui_lang_guest::wire::Border { color: ::std::option::Option::Some({ let __color: ::iced::Color = __ice_palette.colors[107]; ::ui_lang_guest::wire::Rgba([__color.r, __color.g, __color.b, __color.a]) }), width: ::std::option::Option::Some(((1.0) as f32).max(0.0).min(f32::MAX)), radius: ::std::option::Option::Some([((7.0) as f32).max(0.0).min(f32::MAX), ((7.0) as f32).max(0.0).min(f32::MAX), ((7.0) as f32).max(0.0).min(f32::MAX), ((7.0) as f32).max(0.0).min(f32::MAX)]) }), snap: ::std::option::Option::None, content: ::std::boxed::Box::new({
#[cfg(test)]
let __ice_render_source_location = ::ui_lang_runtime::testing::Location::new("src/ui/pages.ice", 375, 1, "rendered view node");
#[cfg(test)]
let __ice_render_source = ::ui_lang_runtime::testing::push_render_source(__ice_render_source_location);
let __ice_rendered: __IceElement<'_, __PagesViewMessage> = ::ui_lang_guest::wire::Node::Text { options: ::ui_lang_guest::wire::TextOptions { height: ::std::option::Option::None, align_y: ::std::option::Option::None, line_height: ::std::option::Option::None, shaping: ::std::option::Option::None, wrapping: ::std::option::Option::Some(::ui_lang_guest::wire::Wrapping::None), tracking: 0.0f32, font: ::std::option::Option::Some(::ui_lang_guest::wire::NamedFont { family: ::ui_lang_guest::wire::FontFamily::Named("Geist Mono".into()), weight: ::ui_lang_guest::wire::Weight::Medium, stretch: ::ui_lang_guest::wire::FontStretch::Normal, style: ::ui_lang_guest::wire::FontStyle::Normal }) }, key: format!("{}/@text:375", __ice_use_scope), size: ::std::option::Option::Some(((10.5) as f32).max(f32::EPSILON).min(f32::MAX)), color: ::std::option::Option::Some({ let __color: ::iced::Color = __ice_palette.colors[75]; ::ui_lang_guest::wire::Rgba([__color.r, __color.g, __color.b, __color.a]) }), font: ::ui_lang_guest::wire::Font { monospace: false, weight: ::ui_lang_guest::wire::Weight::Normal }, width: ::std::option::Option::None, align_x: ::std::option::Option::None, content: ("✓ synced".to_owned()).to_string() };
#[cfg(test)]
drop(__ice_render_source);
__ice_rendered
}) };
#[cfg(test)]
drop(__ice_render_source);
__ice_rendered
}); } ::ui_lang_guest::wire::Node::Linear { max_width: ::std::option::Option::None, clip: false, key: format!("{}/@layout:170", __ice_use_scope), wrap: None, axis: ::ui_lang_guest::wire::Axis::Row, spacing: ::std::option::Option::Some((9.0) as f32), padding: ::std::option::Option::None, width: ::std::option::Option::Some(::ui_lang_guest::wire::Length::Fill), height: ::std::option::Option::Some(::ui_lang_guest::wire::Length::Fill), align: ::std::option::Option::Some(::ui_lang_guest::wire::AlignX::Center), background: ::std::option::Option::None, border: ::std::option::Option::None, children: __children } };
#[cfg(test)]
drop(__ice_render_source);
__ice_rendered
}) };
#[cfg(test)]
drop(__ice_render_source);
__ice_rendered
}); __children.push({
#[cfg(test)]
let __ice_render_source_location = ::ui_lang_runtime::testing::Location::new("src/ui/pages.ice", 381, 1, "rendered view node");
#[cfg(test)]
let __ice_render_source = ::ui_lang_runtime::testing::push_render_source(__ice_render_source_location);
let __ice_rendered: __IceElement<'_, __PagesViewMessage> = ::ui_lang_guest::wire::Node::Container { shadow: ::ui_lang_guest::wire::Shadow { color: ::std::option::Option::None, x: ::std::option::Option::None, y: ::std::option::Option::None, blur: ::std::option::Option::None }, max_width: ::std::option::Option::None, max_height: ::std::option::Option::None, clip: false, key: format!("{}/@container:381", __ice_use_scope), width: ::std::option::Option::Some(::ui_lang_guest::wire::Length::Fill), height: ::std::option::Option::Some(::ui_lang_guest::wire::Length::Fixed((1.0) as f32)), padding: ::std::option::Option::None, align_x: ::std::option::Option::None, align_y: ::std::option::Option::None, background: (::std::option::Option::Some({ let __color: ::iced::Color = __ice_palette.colors[60]; ::ui_lang_guest::wire::Rgba([__color.r, __color.g, __color.b, __color.a]) })).map(::ui_lang_guest::wire::Background::Color), border: ::std::option::Option::None, snap: ::std::option::Option::None, content: ::std::boxed::Box::new({
#[cfg(test)]
let __ice_render_source_location = ::ui_lang_runtime::testing::Location::new("src/ui/pages.ice", 386, 1, "rendered view node");
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
}); ::ui_lang_guest::wire::Node::Linear { max_width: ::std::option::Option::None, clip: false, key: format!("{}/@layout:163", __ice_use_scope), wrap: None, axis: ::ui_lang_guest::wire::Axis::Column, spacing: ::std::option::Option::None, padding: ::std::option::Option::None, width: ::std::option::Option::Some(::ui_lang_guest::wire::Length::Fill), height: ::std::option::Option::None, align: ::std::option::Option::None, background: ::std::option::Option::None, border: ::std::option::Option::None, children: __children } };
#[cfg(test)]
drop(__ice_render_source);
__ice_rendered
}); } __children.push({
#[cfg(test)]
let __ice_render_source_location = ::ui_lang_runtime::testing::Location::new("src/ui/pages.ice", 387, 1, "rendered view node");
#[cfg(test)]
let __ice_render_source = ::ui_lang_runtime::testing::push_render_source(__ice_render_source_location);
let __ice_rendered: __IceElement<'_, __PagesViewMessage> = { let mut __children: Vec<__IceElement<'_, __PagesViewMessage>> = Vec::new(); __children.push({
#[cfg(test)]
let __ice_render_source_location = ::ui_lang_runtime::testing::Location::new("src/ui/pages.ice", 396, 1, "rendered view node");
#[cfg(test)]
let __ice_render_source = ::ui_lang_runtime::testing::push_render_source(__ice_render_source_location);
let __ice_rendered: __IceElement<'_, __PagesViewMessage> = { let __ice_node_scope = format!("{}/pane-measure", __ice_use_scope); ::ui_lang_guest::wire::Node::Sensor { key: __ice_node_scope.clone(), reset: ::std::option::Option::None, on_show: ::std::option::Option::Some(::ui_lang_guest::slots::handler::<(f32, f32), __PagesViewMessage>(::std::boxed::Box::new({ let __route = {  { let __route_callback = (move |__event_0, __event_1| __PagesViewMessage::PagesPaneResized(__event_0, __event_1)).clone(); move |__size: (f64, f64)| (__route_callback)(__size.0, __size.1) } }; move |__sent: (f32, f32)| ::std::option::Option::Some(__route((f64::from(__sent.0), f64::from(__sent.1)))) }))), on_resize: ::std::option::Option::Some(::ui_lang_guest::slots::handler::<(f32, f32), __PagesViewMessage>(::std::boxed::Box::new({ let __route = {  { let __route_callback = (move |__event_0, __event_1| __PagesViewMessage::PagesPaneResized(__event_0, __event_1)).clone(); move |__size: (f64, f64)| (__route_callback)(__size.0, __size.1) } }; move |__sent: (f32, f32)| ::std::option::Option::Some(__route((f64::from(__sent.0), f64::from(__sent.1)))) }))), on_hide: ::std::option::Option::None, anticipate: ::std::option::Option::None, delay: ::std::option::Option::None, child: ::std::boxed::Box::new({
#[cfg(test)]
let __ice_render_source_location = ::ui_lang_runtime::testing::Location::new("src/ui/pages.ice", 397, 1, "rendered view node");
#[cfg(test)]
let __ice_render_source = ::ui_lang_runtime::testing::push_render_source(__ice_render_source_location);
let __ice_rendered: __IceElement<'_, __PagesViewMessage> = ::ui_lang_guest::wire::Node::Space { width: ::std::option::Option::Some(::ui_lang_guest::wire::Length::Fill), height: ::std::option::Option::Some(::ui_lang_guest::wire::Length::Fill) };
#[cfg(test)]
drop(__ice_render_source);
__ice_rendered
}) } };
#[cfg(test)]
drop(__ice_render_source);
__ice_rendered
}); if (!self.connected) { if (self.host_error).is_empty() { __children.push({
#[cfg(test)]
let __ice_render_source_location = ::ui_lang_runtime::testing::Location::new("src/ui/pages.ice", 400, 1, "rendered view node");
#[cfg(test)]
let __ice_render_source = ::ui_lang_runtime::testing::push_render_source(__ice_render_source_location);
let __ice_rendered: __IceElement<'_, __PagesViewMessage> = ::ui_lang_runtime::grow_stack(|| self.__ice_component_use_6(__ice_palette, format!("{}/EmptyState@1021", __ice_use_scope)));
#[cfg(test)]
drop(__ice_render_source);
__ice_rendered
}); } } if ((((self.host_error).is_empty() && self.connected) && self.loading) && (self.active_page).is_empty()) { __children.push({
#[cfg(test)]
let __ice_render_source_location = ::ui_lang_runtime::testing::Location::new("src/ui/pages.ice", 405, 1, "rendered view node");
#[cfg(test)]
let __ice_render_source = ::ui_lang_runtime::testing::push_render_source(__ice_render_source_location);
let __ice_rendered: __IceElement<'_, __PagesViewMessage> = ::ui_lang_runtime::grow_stack(|| self.__ice_component_use_7(__ice_palette, format!("{}/EmptyState@1026", __ice_use_scope)));
#[cfg(test)]
drop(__ice_render_source);
__ice_rendered
}); } if ((((self.host_error).is_empty() && self.connected) && (!self.loading)) && (self.active_page).is_empty()) { __children.push({
#[cfg(test)]
let __ice_render_source_location = ::ui_lang_runtime::testing::Location::new("src/ui/pages.ice", 407, 1, "rendered view node");
#[cfg(test)]
let __ice_render_source = ::ui_lang_runtime::testing::push_render_source(__ice_render_source_location);
let __ice_rendered: __IceElement<'_, __PagesViewMessage> = ::ui_lang_runtime::grow_stack(|| self.__ice_component_use_8(__ice_palette, format!("{}/EmptyState@1028", __ice_use_scope)));
#[cfg(test)]
drop(__ice_render_source);
__ice_rendered
}); } if (self.connected && (!(self.active_page).is_empty())) { __children.push({
#[cfg(test)]
let __ice_render_source_location = ::ui_lang_runtime::testing::Location::new("src/ui/pages.ice", 436, 1, "rendered view node");
#[cfg(test)]
let __ice_render_source = ::ui_lang_runtime::testing::push_render_source(__ice_render_source_location);
let __ice_rendered: __IceElement<'_, __PagesViewMessage> = ::ui_lang_guest::wire::Node::Container { shadow: ::ui_lang_guest::wire::Shadow { color: ::std::option::Option::None, x: ::std::option::Option::None, y: ::std::option::Option::None, blur: ::std::option::Option::None }, max_width: ::std::option::Option::Some(((crate::host::document_width(self.pages_pane_width, self.block_comments_open)) as f32).max(0.0).min(f32::MAX)), max_height: ::std::option::Option::None, clip: false, key: format!("{}/@container:436", __ice_use_scope), width: ::std::option::Option::Some(::ui_lang_guest::wire::Length::Fill), height: ::std::option::Option::Some(::ui_lang_guest::wire::Length::Fill), padding: ::std::option::Option::Some(::ui_lang_guest::wire::Edges { top: (26.0) as f32, right: (40.0) as f32, bottom: (18.0) as f32, left: (22.0) as f32 }), align_x: ::std::option::Option::None, align_y: ::std::option::Option::None, background: (::std::option::Option::None).map(::ui_lang_guest::wire::Background::Color), border: ::std::option::Option::None, snap: ::std::option::Option::None, content: ::std::boxed::Box::new({
#[cfg(test)]
let __ice_render_source_location = ::ui_lang_runtime::testing::Location::new("src/ui/pages.ice", 445, 1, "rendered view node");
#[cfg(test)]
let __ice_render_source = ::ui_lang_runtime::testing::push_render_source(__ice_render_source_location);
let __ice_rendered: __IceElement<'_, __PagesViewMessage> = { let mut __children: ::std::vec::Vec<__IceElement<'_, __PagesViewMessage>> = ::std::vec::Vec::new(); if (!(self.page_refusal).is_empty()) { __children.push({
#[cfg(test)]
let __ice_render_source_location = ::ui_lang_runtime::testing::Location::new("src/ui/pages.ice", 455, 1, "rendered view node");
#[cfg(test)]
let __ice_render_source = ::ui_lang_runtime::testing::push_render_source(__ice_render_source_location);
let __ice_rendered: __IceElement<'_, __PagesViewMessage> = ::ui_lang_guest::wire::Node::Container { shadow: ::ui_lang_guest::wire::Shadow { color: ::std::option::Option::None, x: ::std::option::Option::None, y: ::std::option::Option::None, blur: ::std::option::Option::None }, max_width: ::std::option::Option::None, max_height: ::std::option::Option::None, clip: false, key: format!("{}/@container:455", __ice_use_scope), width: ::std::option::Option::Some(::ui_lang_guest::wire::Length::Fill), height: ::std::option::Option::None, padding: ::std::option::Option::Some(::ui_lang_guest::wire::Edges { top: (9.0) as f32, right: (12.0) as f32, bottom: (9.0) as f32, left: (12.0) as f32 }), align_x: ::std::option::Option::None, align_y: ::std::option::Option::None, background: (::std::option::Option::Some({ let __color: ::iced::Color = __ice_palette.colors[81]; ::ui_lang_guest::wire::Rgba([__color.r, __color.g, __color.b, __color.a]) })).map(::ui_lang_guest::wire::Background::Color), border: ::std::option::Option::Some(::ui_lang_guest::wire::Border { color: ::std::option::Option::Some({ let __color: ::iced::Color = __ice_palette.colors[82]; ::ui_lang_guest::wire::Rgba([__color.r, __color.g, __color.b, __color.a]) }), width: ::std::option::Option::Some(((1.0) as f32).max(0.0).min(f32::MAX)), radius: ::std::option::Option::Some([((9.0) as f32).max(0.0).min(f32::MAX), ((9.0) as f32).max(0.0).min(f32::MAX), ((9.0) as f32).max(0.0).min(f32::MAX), ((9.0) as f32).max(0.0).min(f32::MAX)]) }), snap: ::std::option::Option::None, content: ::std::boxed::Box::new({
#[cfg(test)]
let __ice_render_source_location = ::ui_lang_runtime::testing::Location::new("src/ui/pages.ice", 464, 1, "rendered view node");
#[cfg(test)]
let __ice_render_source = ::ui_lang_runtime::testing::push_render_source(__ice_render_source_location);
let __ice_rendered: __IceElement<'_, __PagesViewMessage> = ::ui_lang_guest::wire::Node::Text { options: ::ui_lang_guest::wire::TextOptions { height: ::std::option::Option::None, align_y: ::std::option::Option::None, line_height: ::std::option::Option::None, shaping: ::std::option::Option::None, wrapping: ::std::option::Option::Some(::ui_lang_guest::wire::Wrapping::Word), tracking: 0.0f32, font: ::std::option::Option::Some(::ui_lang_guest::wire::NamedFont { family: ::ui_lang_guest::wire::FontFamily::Named("Geist".into()), weight: ::ui_lang_guest::wire::Weight::Normal, stretch: ::ui_lang_guest::wire::FontStretch::Normal, style: ::ui_lang_guest::wire::FontStyle::Normal }) }, key: format!("{}/@text:464", __ice_use_scope), size: ::std::option::Option::Some(((12.5) as f32).max(f32::EPSILON).min(f32::MAX)), color: ::std::option::Option::Some({ let __color: ::iced::Color = __ice_palette.colors[80]; ::ui_lang_guest::wire::Rgba([__color.r, __color.g, __color.b, __color.a]) }), font: ::ui_lang_guest::wire::Font { monospace: false, weight: ::ui_lang_guest::wire::Weight::Normal }, width: ::std::option::Option::Some(::ui_lang_guest::wire::Length::Fill), align_x: ::std::option::Option::None, content: (self.page_refusal.to_owned()).to_string() };
#[cfg(test)]
drop(__ice_render_source);
__ice_rendered
}) };
#[cfg(test)]
drop(__ice_render_source);
__ice_rendered
}); } if (!(self.orphaned_comment_drafts).is_empty()) { __children.push({
#[cfg(test)]
let __ice_render_source_location = ::ui_lang_runtime::testing::Location::new("src/ui/pages.ice", 471, 1, "rendered view node");
#[cfg(test)]
let __ice_render_source = ::ui_lang_runtime::testing::push_render_source(__ice_render_source_location);
let __ice_rendered: __IceElement<'_, __PagesViewMessage> = ::ui_lang_guest::wire::Node::Container { shadow: ::ui_lang_guest::wire::Shadow { color: ::std::option::Option::None, x: ::std::option::Option::None, y: ::std::option::Option::None, blur: ::std::option::Option::None }, max_width: ::std::option::Option::None, max_height: ::std::option::Option::None, clip: false, key: format!("{}/@container:471", __ice_use_scope), width: ::std::option::Option::Some(::ui_lang_guest::wire::Length::Fill), height: ::std::option::Option::None, padding: ::std::option::Option::Some(::ui_lang_guest::wire::Edges { top: (7.0) as f32, right: (7.0) as f32, bottom: (7.0) as f32, left: (7.0) as f32 }), align_x: ::std::option::Option::None, align_y: ::std::option::Option::None, background: (::std::option::Option::Some({ let __color: ::iced::Color = __ice_palette.colors[55]; ::ui_lang_guest::wire::Rgba([__color.r, __color.g, __color.b, __color.a]) })).map(::ui_lang_guest::wire::Background::Color), border: ::std::option::Option::Some(::ui_lang_guest::wire::Border { color: ::std::option::Option::Some({ let __color: ::iced::Color = { let mut __color = __ice_palette.colors[4]; __color.a = 0.090000; __color }; ::ui_lang_guest::wire::Rgba([__color.r, __color.g, __color.b, __color.a]) }), width: ::std::option::Option::Some(((1.0) as f32).max(0.0).min(f32::MAX)), radius: ::std::option::Option::Some([((9.0) as f32).max(0.0).min(f32::MAX), ((9.0) as f32).max(0.0).min(f32::MAX), ((9.0) as f32).max(0.0).min(f32::MAX), ((9.0) as f32).max(0.0).min(f32::MAX)]) }), snap: ::std::option::Option::None, content: ::std::boxed::Box::new({
#[cfg(test)]
let __ice_render_source_location = ::ui_lang_runtime::testing::Location::new("src/ui/pages.ice", 479, 1, "rendered view node");
#[cfg(test)]
let __ice_render_source = ::ui_lang_runtime::testing::push_render_source(__ice_render_source_location);
let __ice_rendered: __IceElement<'_, __PagesViewMessage> = { let mut __children: ::std::vec::Vec<__IceElement<'_, __PagesViewMessage>> = ::std::vec::Vec::new(); __children.push({
#[cfg(test)]
let __ice_render_source_location = ::ui_lang_runtime::testing::Location::new("src/ui/pages.ice", 480, 1, "rendered view node");
#[cfg(test)]
let __ice_render_source = ::ui_lang_runtime::testing::push_render_source(__ice_render_source_location);
let __ice_rendered: __IceElement<'_, __PagesViewMessage> = ::ui_lang_guest::wire::Node::Text { options: ::ui_lang_guest::wire::TextOptions { height: ::std::option::Option::None, align_y: ::std::option::Option::None, line_height: ::std::option::Option::None, shaping: ::std::option::Option::None, wrapping: ::std::option::Option::None, tracking: 0.0f32, font: ::std::option::Option::Some(::ui_lang_guest::wire::NamedFont { family: ::ui_lang_guest::wire::FontFamily::Named("Geist".into()), weight: ::ui_lang_guest::wire::Weight::Medium, stretch: ::ui_lang_guest::wire::FontStretch::Normal, style: ::ui_lang_guest::wire::FontStyle::Normal }) }, key: format!("{}/@text:480", __ice_use_scope), size: ::std::option::Option::Some(((13.0) as f32).max(f32::EPSILON).min(f32::MAX)), color: ::std::option::Option::Some({ let __color: ::iced::Color = __ice_palette.colors[4]; ::ui_lang_guest::wire::Rgba([__color.r, __color.g, __color.b, __color.a]) }), font: ::ui_lang_guest::wire::Font { monospace: false, weight: ::ui_lang_guest::wire::Weight::Normal }, width: ::std::option::Option::None, align_x: ::std::option::Option::None, content: ("Recovered drafts".to_owned()).to_string() };
#[cfg(test)]
drop(__ice_render_source);
__ice_rendered
}); for (__ice_index, recovered_comment) in self.orphaned_comment_drafts.iter().enumerate() { let __for_scope = format!("{}/@for:1106({})", __ice_use_scope, __ice_index); __children.push({
#[cfg(test)]
let __ice_render_source_location = ::ui_lang_runtime::testing::Location::new("src/ui/pages.ice", 486, 1, "rendered view node");
#[cfg(test)]
let __ice_render_source = ::ui_lang_runtime::testing::push_render_source(__ice_render_source_location);
let __ice_rendered: __IceElement<'_, __PagesViewMessage> = { let mut __children: ::std::vec::Vec<__IceElement<'_, __PagesViewMessage>> = ::std::vec::Vec::new(); __children.push({
#[cfg(test)]
let __ice_render_source_location = ::ui_lang_runtime::testing::Location::new("src/ui/pages.ice", 491, 1, "rendered view node");
#[cfg(test)]
let __ice_render_source = ::ui_lang_runtime::testing::push_render_source(__ice_render_source_location);
let __ice_rendered: __IceElement<'_, __PagesViewMessage> = ::ui_lang_guest::wire::Node::Text { options: ::ui_lang_guest::wire::TextOptions { height: ::std::option::Option::None, align_y: ::std::option::Option::None, line_height: ::std::option::Option::None, shaping: ::std::option::Option::None, wrapping: ::std::option::Option::None, tracking: 0.0f32, font: ::std::option::Option::Some(::ui_lang_guest::wire::NamedFont { family: ::ui_lang_guest::wire::FontFamily::Named("Geist".into()), weight: ::ui_lang_guest::wire::Weight::Normal, stretch: ::ui_lang_guest::wire::FontStretch::Normal, style: ::ui_lang_guest::wire::FontStyle::Normal }) }, key: format!("{}/@text:491", __for_scope), size: ::std::option::Option::Some(((13.5) as f32).max(f32::EPSILON).min(f32::MAX)), color: ::std::option::Option::Some({ let __color: ::iced::Color = __ice_palette.colors[5]; ::ui_lang_guest::wire::Rgba([__color.r, __color.g, __color.b, __color.a]) }), font: ::ui_lang_guest::wire::Font { monospace: false, weight: ::ui_lang_guest::wire::Weight::Normal }, width: ::std::option::Option::Some(::ui_lang_guest::wire::Length::Fill), align_x: ::std::option::Option::None, content: (recovered_comment.to_owned()).to_string() };
#[cfg(test)]
drop(__ice_render_source);
__ice_rendered
}); __children.push({
#[cfg(test)]
let __ice_render_source_location = ::ui_lang_runtime::testing::Location::new("src/ui/pages.ice", 496, 1, "rendered view node");
#[cfg(test)]
let __ice_render_source = ::ui_lang_runtime::testing::push_render_source(__ice_render_source_location);
let __ice_rendered: __IceElement<'_, __PagesViewMessage> = ::ui_lang_guest::wire::Node::Button { checked: ::std::option::Option::None, expanded: ::std::option::Option::None, description: ::std::option::Option::None, key: format!("{}/@button:496", __for_scope), content: ::ui_lang_guest::wire::ButtonContent::Label(::std::string::String::from("Use")), label: ::std::option::Option::Some(::std::string::String::from("Use as comment".to_owned())), on_press: if ((self.loading || (self.busy || (!(self.host_error).is_empty())))) { ::std::option::Option::None } else { ::std::option::Option::Some(::ui_lang_guest::slots::message((move |__event_0| __PagesViewMessage::UseOrphanedCommentDraft(__event_0))(recovered_comment.to_owned()))) }, width: ::std::option::Option::None, height: ::std::option::Option::None, padding: ::std::option::Option::Some(::ui_lang_guest::wire::Edges::all((5.0) as f32)), style: ::ui_lang_guest::wire::ButtonStyle { preset: ::ui_lang_guest::wire::ButtonPreset::Primary, recipe: Some(::ui_lang_guest::wire::ButtonRecipe { base: ::ui_lang_guest::wire::Face { background: ::std::option::Option::Some({ let __color: ::iced::Color = ::iced::Color::from_rgba8(0, 0, 0, 0.000000); ::ui_lang_guest::wire::Rgba([__color.r, __color.g, __color.b, __color.a]) }), text: ::std::option::Option::Some({ let __color: ::iced::Color = __ice_palette.colors[4]; ::ui_lang_guest::wire::Rgba([__color.r, __color.g, __color.b, __color.a]) }), border: ::std::option::Option::Some(::ui_lang_guest::wire::Border { color: ::std::option::Option::None, width: ::std::option::Option::None, radius: ::std::option::Option::Some([8.0; 4]) }) }, hover_background: ::std::option::Option::Some({ let __color: ::iced::Color = __ice_palette.colors[14]; ::ui_lang_guest::wire::Rgba([__color.r, __color.g, __color.b, __color.a]) }), pressed_background: ::std::option::Option::Some({ let __color: ::iced::Color = __ice_palette.colors[39]; ::ui_lang_guest::wire::Rgba([__color.r, __color.g, __color.b, __color.a]) }), disabled_background: ::std::option::Option::None, disabled_text: ::std::option::Option::None, disabled_opacity: ::std::option::Option::Some(0.5f32), focus_ring: ::std::option::Option::Some({ let __color: ::iced::Color = __ice_palette.colors[42]; ::ui_lang_guest::wire::Rgba([__color.r, __color.g, __color.b, __color.a]) }), text_size: ::std::option::Option::Some(12.5f32), line_height: ::std::option::Option::None, font: Some(::ui_lang_guest::wire::NamedFont { family: ::ui_lang_guest::wire::FontFamily::Named("Geist".into()), weight: ::ui_lang_guest::wire::Weight::Semibold, stretch: ::ui_lang_guest::wire::FontStretch::Normal, style: ::ui_lang_guest::wire::FontStyle::Normal }) }), active: ::ui_lang_guest::wire::Face { background: ::std::option::Option::Some({ let __color: ::iced::Color = { let mut __color = __ice_palette.colors[4]; __color.a = 0.090000; __color }; ::ui_lang_guest::wire::Rgba([__color.r, __color.g, __color.b, __color.a]) }), text: ::std::option::Option::Some({ let __color: ::iced::Color = __ice_palette.colors[4]; ::ui_lang_guest::wire::Rgba([__color.r, __color.g, __color.b, __color.a]) }), border: ::std::option::Option::Some(::ui_lang_guest::wire::Border { color: ::std::option::Option::Some({ let __color: ::iced::Color = { let mut __color = __ice_palette.colors[4]; __color.a = 0.120000; __color }; ::ui_lang_guest::wire::Rgba([__color.r, __color.g, __color.b, __color.a]) }), width: ::std::option::Option::Some(((1.0) as f32).max(0.0).min(f32::MAX)), radius: ::std::option::Option::Some([((7.0) as f32).max(0.0).min(f32::MAX), ((7.0) as f32).max(0.0).min(f32::MAX), ((7.0) as f32).max(0.0).min(f32::MAX), ((7.0) as f32).max(0.0).min(f32::MAX)]) }) }, hovered: ::std::option::Option::Some(::ui_lang_guest::wire::Face { background: ::std::option::Option::Some({ let __color: ::iced::Color = { let mut __color = __ice_palette.colors[4]; __color.a = 0.140000; __color }; ::ui_lang_guest::wire::Rgba([__color.r, __color.g, __color.b, __color.a]) }), text: ::std::option::Option::None, border: ::std::option::Option::None }), pressed: ::std::option::Option::Some(::ui_lang_guest::wire::Face { background: ::std::option::Option::Some({ let __color: ::iced::Color = { let mut __color = __ice_palette.colors[4]; __color.a = 0.180000; __color }; ::ui_lang_guest::wire::Rgba([__color.r, __color.g, __color.b, __color.a]) }), text: ::std::option::Option::None, border: ::std::option::Option::None }), disabled: ::std::option::Option::None } };
#[cfg(test)]
drop(__ice_render_source);
__ice_rendered
}); __children.push({
#[cfg(test)]
let __ice_render_source_location = ::ui_lang_runtime::testing::Location::new("src/ui/pages.ice", 505, 1, "rendered view node");
#[cfg(test)]
let __ice_render_source = ::ui_lang_runtime::testing::push_render_source(__ice_render_source_location);
let __ice_rendered: __IceElement<'_, __PagesViewMessage> = ::ui_lang_guest::wire::Node::Button { checked: ::std::option::Option::None, expanded: ::std::option::Option::None, description: ::std::option::Option::None, key: format!("{}/@button:505", __for_scope), content: ::ui_lang_guest::wire::ButtonContent::Label(::std::string::String::from("Discard")), label: ::std::option::Option::None, on_press: if ((self.loading || (self.busy || (!(self.host_error).is_empty())))) { ::std::option::Option::None } else { ::std::option::Option::Some(::ui_lang_guest::slots::message((move |__event_0| __PagesViewMessage::DiscardOrphanedCommentDraft(__event_0))(recovered_comment.to_owned()))) }, width: ::std::option::Option::None, height: ::std::option::Option::None, padding: ::std::option::Option::Some(::ui_lang_guest::wire::Edges::all((5.0) as f32)), style: ::ui_lang_guest::wire::ButtonStyle { preset: ::ui_lang_guest::wire::ButtonPreset::Primary, recipe: Some(::ui_lang_guest::wire::ButtonRecipe { base: ::ui_lang_guest::wire::Face { background: ::std::option::Option::Some({ let __color: ::iced::Color = __ice_palette.colors[20]; ::ui_lang_guest::wire::Rgba([__color.r, __color.g, __color.b, __color.a]) }), text: ::std::option::Option::Some({ let __color: ::iced::Color = __ice_palette.colors[21]; ::ui_lang_guest::wire::Rgba([__color.r, __color.g, __color.b, __color.a]) }), border: ::std::option::Option::Some(::ui_lang_guest::wire::Border { color: ::std::option::Option::None, width: ::std::option::Option::None, radius: ::std::option::Option::Some([9.0; 4]) }) }, hover_background: ::std::option::Option::Some({ let __color: ::iced::Color = { let mut __color = __ice_palette.colors[20]; __color.a = 0.900000; __color }; ::ui_lang_guest::wire::Rgba([__color.r, __color.g, __color.b, __color.a]) }), pressed_background: ::std::option::Option::Some({ let __color: ::iced::Color = { let mut __color = __ice_palette.colors[20]; __color.a = 0.800000; __color }; ::ui_lang_guest::wire::Rgba([__color.r, __color.g, __color.b, __color.a]) }), disabled_background: ::std::option::Option::None, disabled_text: ::std::option::Option::None, disabled_opacity: ::std::option::Option::Some(0.5f32), focus_ring: ::std::option::Option::Some({ let __color: ::iced::Color = __ice_palette.colors[42]; ::ui_lang_guest::wire::Rgba([__color.r, __color.g, __color.b, __color.a]) }), text_size: ::std::option::Option::Some(12.5f32), line_height: ::std::option::Option::None, font: Some(::ui_lang_guest::wire::NamedFont { family: ::ui_lang_guest::wire::FontFamily::Named("Geist".into()), weight: ::ui_lang_guest::wire::Weight::Semibold, stretch: ::ui_lang_guest::wire::FontStretch::Normal, style: ::ui_lang_guest::wire::FontStyle::Normal }) }), active: ::ui_lang_guest::wire::Face::default(), hovered: ::std::option::Option::None, pressed: ::std::option::Option::None, disabled: ::std::option::Option::None } };
#[cfg(test)]
drop(__ice_render_source);
__ice_rendered
}); ::ui_lang_guest::wire::Node::Linear { max_width: ::std::option::Option::None, clip: false, key: format!("{}/@layout:486", __for_scope), wrap: None, axis: ::ui_lang_guest::wire::Axis::Row, spacing: ::std::option::Option::Some((5.0) as f32), padding: ::std::option::Option::None, width: ::std::option::Option::Some(::ui_lang_guest::wire::Length::Fill), height: ::std::option::Option::None, align: ::std::option::Option::Some(::ui_lang_guest::wire::AlignX::Center), background: ::std::option::Option::None, border: ::std::option::Option::None, children: __children } };
#[cfg(test)]
drop(__ice_render_source);
__ice_rendered
}); } ::ui_lang_guest::wire::Node::Linear { max_width: ::std::option::Option::None, clip: false, key: format!("{}/@layout:479", __ice_use_scope), wrap: None, axis: ::ui_lang_guest::wire::Axis::Column, spacing: ::std::option::Option::Some((5.0) as f32), padding: ::std::option::Option::None, width: ::std::option::Option::Some(::ui_lang_guest::wire::Length::Fill), height: ::std::option::Option::None, align: ::std::option::Option::None, background: ::std::option::Option::None, border: ::std::option::Option::None, children: __children } };
#[cfg(test)]
drop(__ice_render_source);
__ice_rendered
}) };
#[cfg(test)]
drop(__ice_render_source);
__ice_rendered
}); } __children.push({
#[cfg(test)]
let __ice_render_source_location = ::ui_lang_runtime::testing::Location::new("src/ui/pages.ice", 514, 1, "rendered view node");
#[cfg(test)]
let __ice_render_source = ::ui_lang_runtime::testing::push_render_source(__ice_render_source_location);
let __ice_rendered: __IceElement<'_, __PagesViewMessage> = (|| { let __slot_content: __IceElement<'_, __PagesViewMessage> = {
#[cfg(test)]
let __ice_render_source_location = ::ui_lang_runtime::testing::Location::new("src/ui/app.ice", 799, 1, "rendered view node");
#[cfg(test)]
let __ice_render_source = ::ui_lang_runtime::testing::push_render_source(__ice_render_source_location);
let __ice_rendered: __IceElement<'_, __PagesViewMessage> = { let mut __children: ::std::vec::Vec<__IceElement<'_, __PagesViewMessage>> = ::std::vec::Vec::new(); if (!(crate::editor_view::presentation_notice(::std::borrow::Borrow::borrow(&(self.document)), ::std::borrow::Borrow::borrow(&(self.document_paint)))).is_empty()) { __children.push({
#[cfg(test)]
let __ice_render_source_location = ::ui_lang_runtime::testing::Location::new("src/ui/app.ice", 801, 1, "rendered view node");
#[cfg(test)]
let __ice_render_source = ::ui_lang_runtime::testing::push_render_source(__ice_render_source_location);
let __ice_rendered: __IceElement<'_, __PagesViewMessage> = ::ui_lang_guest::wire::Node::Text { options: ::ui_lang_guest::wire::TextOptions { height: ::std::option::Option::None, align_y: ::std::option::Option::None, line_height: ::std::option::Option::None, shaping: ::std::option::Option::None, wrapping: ::std::option::Option::None, tracking: 0.0f32, font: ::std::option::Option::Some(::ui_lang_guest::wire::NamedFont { family: ::ui_lang_guest::wire::FontFamily::Named("Geist".into()), weight: ::ui_lang_guest::wire::Weight::Normal, stretch: ::ui_lang_guest::wire::FontStretch::Normal, style: ::ui_lang_guest::wire::FontStyle::Normal }) }, key: format!("{}/@text:801", __ice_use_scope), size: ::std::option::Option::Some(13.5f32), color: ::std::option::Option::Some({ let __color: ::iced::Color = __ice_palette.colors[5]; ::ui_lang_guest::wire::Rgba([__color.r, __color.g, __color.b, __color.a]) }), font: ::ui_lang_guest::wire::Font { monospace: false, weight: ::ui_lang_guest::wire::Weight::Normal }, width: ::std::option::Option::None, align_x: ::std::option::Option::None, content: (crate::editor_view::presentation_notice(::std::borrow::Borrow::borrow(&(self.document)), ::std::borrow::Borrow::borrow(&(self.document_paint)))).to_string() };
#[cfg(test)]
drop(__ice_render_source);
__ice_rendered
}); } if (!(self.document_error).is_empty()) { __children.push({
#[cfg(test)]
let __ice_render_source_location = ::ui_lang_runtime::testing::Location::new("src/ui/app.ice", 803, 1, "rendered view node");
#[cfg(test)]
let __ice_render_source = ::ui_lang_runtime::testing::push_render_source(__ice_render_source_location);
let __ice_rendered: __IceElement<'_, __PagesViewMessage> = ::ui_lang_guest::wire::Node::Text { options: ::ui_lang_guest::wire::TextOptions { height: ::std::option::Option::None, align_y: ::std::option::Option::None, line_height: ::std::option::Option::None, shaping: ::std::option::Option::None, wrapping: ::std::option::Option::None, tracking: 0.0f32, font: ::std::option::Option::Some(::ui_lang_guest::wire::NamedFont { family: ::ui_lang_guest::wire::FontFamily::Named("Geist".into()), weight: ::ui_lang_guest::wire::Weight::Normal, stretch: ::ui_lang_guest::wire::FontStretch::Normal, style: ::ui_lang_guest::wire::FontStyle::Normal }) }, key: format!("{}/@text:803", __ice_use_scope), size: ::std::option::Option::Some(13.5f32), color: ::std::option::Option::Some({ let __color: ::iced::Color = __ice_palette.colors[20]; ::ui_lang_guest::wire::Rgba([__color.r, __color.g, __color.b, __color.a]) }), font: ::ui_lang_guest::wire::Font { monospace: false, weight: ::ui_lang_guest::wire::Weight::Normal }, width: ::std::option::Option::None, align_x: ::std::option::Option::None, content: (self.document_error.to_owned()).to_string() };
#[cfg(test)]
drop(__ice_render_source);
__ice_rendered
}); } __children.push({
#[cfg(test)]
let __ice_render_source_location = ::ui_lang_runtime::testing::Location::new("src/ui/app.ice", 804, 1, "rendered view node");
#[cfg(test)]
let __ice_render_source = ::ui_lang_runtime::testing::push_render_source(__ice_render_source_location);
let __ice_rendered: __IceElement<'_, __PagesViewMessage> = { let __ice_node_scope = format!("{}/document", __ice_use_scope); { let __editor = &(self.document); let (__document, __on_document) = __editor.document(("app:document").to_owned(), __PagesViewMessage::__EditDocument as fn(::ui_lang_guest::EditorDocumentUpdate) -> __PagesViewMessage); ::ui_lang_guest::wire::Node::Editor { options: ::std::boxed::Box::new(::ui_lang_guest::wire::EditorOptions { binding: ::std::option::Option::Some(::std::boxed::Box::new(crate::editor_binding::keys(self.document_history.clone(), self.document_menu.clone()).register({  move |__value| __PagesViewMessage::DocumentCommitted(__value) }, __PagesViewMessage::__0T646f63756d656e74))), presentation: { let __presentation = crate::editor_view::paint(__editor.state_view(), self.document_paint.clone()); __presentation.validate(__editor.state_view().text).expect("invalid editor presentation"); Some(::std::boxed::Box::new(__presentation)) }, size: ::std::option::Option::Some((14.0) as f32), padding: ::std::option::Option::None, line_height: ::std::option::Option::Some(::ui_lang_guest::wire::LineHeight::Relative(((1.65) as f32).max(f32::EPSILON).min(f32::MAX))), wrapping: ::std::option::Option::Some(::ui_lang_guest::wire::Wrapping::Word), font: ::std::option::Option::Some(::ui_lang_guest::wire::NamedFont { family: ::ui_lang_guest::wire::FontFamily::Named("Geist".into()), weight: ::ui_lang_guest::wire::Weight::Normal, stretch: ::ui_lang_guest::wire::FontStretch::Normal, style: ::ui_lang_guest::wire::FontStyle::Normal }), style: ::ui_lang_guest::wire::InputStyle { active: ::ui_lang_guest::wire::InputFace { icon: ::std::option::Option::None, background: ::std::option::Option::Some({ let __color: ::iced::Color = { let mut __color = __ice_palette.colors[4]; __color.a = 0.000000; __color }; ::ui_lang_guest::wire::Rgba([__color.r, __color.g, __color.b, __color.a]) }), border: ::std::option::Option::Some(::ui_lang_guest::wire::Border { color: ::std::option::Option::None, width: ::std::option::Option::Some(((0.0) as f32).max(0.0).min(f32::MAX)), radius: ::std::option::Option::None }), value: ::std::option::Option::Some({ let __color: ::iced::Color = __ice_palette.colors[0]; ::ui_lang_guest::wire::Rgba([__color.r, __color.g, __color.b, __color.a]) }), placeholder: ::std::option::Option::Some({ let __color: ::iced::Color = __ice_palette.colors[72]; ::ui_lang_guest::wire::Rgba([__color.r, __color.g, __color.b, __color.a]) }), selection: ::std::option::Option::Some({ let __color: ::iced::Color = __ice_palette.colors[1]; ::ui_lang_guest::wire::Rgba([__color.r, __color.g, __color.b, __color.a]) }) }, hovered: ::std::option::Option::None, focused: ::std::option::Option::None, focused_hovered: ::std::option::Option::None, disabled: ::std::option::Option::Some(::ui_lang_guest::wire::InputFace { icon: ::std::option::Option::None, background: ::std::option::Option::Some({ let __color: ::iced::Color = { let mut __color = __ice_palette.colors[4]; __color.a = 0.000000; __color }; ::ui_lang_guest::wire::Rgba([__color.r, __color.g, __color.b, __color.a]) }), border: ::std::option::Option::Some(::ui_lang_guest::wire::Border { color: ::std::option::Option::None, width: ::std::option::Option::Some(((0.0) as f32).max(0.0).min(f32::MAX)), radius: ::std::option::Option::None }), value: ::std::option::Option::Some({ let __color: ::iced::Color = __ice_palette.colors[0]; ::ui_lang_guest::wire::Rgba([__color.r, __color.g, __color.b, __color.a]) }), placeholder: ::std::option::Option::Some({ let __color: ::iced::Color = __ice_palette.colors[72]; ::ui_lang_guest::wire::Rgba([__color.r, __color.g, __color.b, __color.a]) }), selection: ::std::option::Option::Some({ let __color: ::iced::Color = __ice_palette.colors[1]; ::ui_lang_guest::wire::Rgba([__color.r, __color.g, __color.b, __color.a]) }) }), ..::std::default::Default::default() } }), key: __ice_node_scope.clone(), placeholder: "Write something… `#` for a heading, `-` for a list".to_owned(), document: __document, on_document: __on_document, editable: !((((((!(self.host_error).is_empty()) || self.loading) || (!self.connected)) || (self.active_page).is_empty()) || (self.active_page != self.buffer_page))), width: ::std::option::Option::None, height: ::std::option::Option::None, min_height: ::std::option::Option::None, max_height: ::std::option::Option::None } } };
#[cfg(test)]
drop(__ice_render_source);
__ice_rendered
}); ::ui_lang_guest::wire::Node::Linear { max_width: ::std::option::Option::None, clip: false, key: format!("{}/@layout:799", __ice_use_scope), wrap: None, axis: ::ui_lang_guest::wire::Axis::Column, spacing: ::std::option::Option::None, padding: ::std::option::Option::None, width: ::std::option::Option::Some(::ui_lang_guest::wire::Length::Fill), height: ::std::option::Option::Some(::ui_lang_guest::wire::Length::Fill), align: ::std::option::Option::None, background: ::std::option::Option::None, border: ::std::option::Option::None, children: __children } };
#[cfg(test)]
drop(__ice_render_source);
__ice_rendered
}; __slot_content })();
#[cfg(test)]
drop(__ice_render_source);
__ice_rendered
}); if (!(self.subpages).is_empty()) { __children.push({
#[cfg(test)]
let __ice_render_source_location = ::ui_lang_runtime::testing::Location::new("src/ui/pages.ice", 519, 1, "rendered view node");
#[cfg(test)]
let __ice_render_source = ::ui_lang_runtime::testing::push_render_source(__ice_render_source_location);
let __ice_rendered: __IceElement<'_, __PagesViewMessage> = { let mut __children: ::std::vec::Vec<__IceElement<'_, __PagesViewMessage>> = ::std::vec::Vec::new(); __children.push({
#[cfg(test)]
let __ice_render_source_location = ::ui_lang_runtime::testing::Location::new("src/ui/pages.ice", 525, 1, "rendered view node");
#[cfg(test)]
let __ice_render_source = ::ui_lang_runtime::testing::push_render_source(__ice_render_source_location);
let __ice_rendered: __IceElement<'_, __PagesViewMessage> = ::ui_lang_guest::wire::Node::Text { options: ::ui_lang_guest::wire::TextOptions { height: ::std::option::Option::None, align_y: ::std::option::Option::None, line_height: ::std::option::Option::None, shaping: ::std::option::Option::None, wrapping: ::std::option::Option::Some(::ui_lang_guest::wire::Wrapping::None), tracking: 0.0f32, font: ::std::option::Option::Some(::ui_lang_guest::wire::NamedFont { family: ::ui_lang_guest::wire::FontFamily::Named("Geist Mono".into()), weight: ::ui_lang_guest::wire::Weight::Medium, stretch: ::ui_lang_guest::wire::FontStretch::Normal, style: ::ui_lang_guest::wire::FontStyle::Normal }) }, key: format!("{}/@text:525", __ice_use_scope), size: ::std::option::Option::Some(((10.5) as f32).max(f32::EPSILON).min(f32::MAX)), color: ::std::option::Option::Some({ let __color: ::iced::Color = __ice_palette.colors[72]; ::ui_lang_guest::wire::Rgba([__color.r, __color.g, __color.b, __color.a]) }), font: ::ui_lang_guest::wire::Font { monospace: false, weight: ::ui_lang_guest::wire::Weight::Normal }, width: ::std::option::Option::None, align_x: ::std::option::Option::None, content: ("Subpages".to_owned()).to_string() };
#[cfg(test)]
drop(__ice_render_source);
__ice_rendered
}); for (__ice_index, child) in self.subpages.iter().enumerate() { let __for_scope = format!("{}/@for:1152({})", __ice_use_scope, __ice_index); __children.push({
#[cfg(test)]
let __ice_render_source_location = ::ui_lang_runtime::testing::Location::new("src/ui/pages.ice", 532, 1, "rendered view node");
#[cfg(test)]
let __ice_render_source = ::ui_lang_runtime::testing::push_render_source(__ice_render_source_location);
let __ice_rendered: __IceElement<'_, __PagesViewMessage> = ::ui_lang_guest::wire::Node::Button { checked: ::std::option::Option::None, expanded: ::std::option::Option::None, description: ::std::option::Option::Some(::std::string::String::from(child.title.to_owned())), key: format!("{}/@button:532", __for_scope), content: ::ui_lang_guest::wire::ButtonContent::Child(::std::boxed::Box::new({
#[cfg(test)]
let __ice_render_source_location = ::ui_lang_runtime::testing::Location::new("src/ui/pages.ice", 540, 1, "rendered view node");
#[cfg(test)]
let __ice_render_source = ::ui_lang_runtime::testing::push_render_source(__ice_render_source_location);
let __ice_rendered: __IceElement<'_, __PagesViewMessage> = { let mut __children: ::std::vec::Vec<__IceElement<'_, __PagesViewMessage>> = ::std::vec::Vec::new(); __children.push({
#[cfg(test)]
let __ice_render_source_location = ::ui_lang_runtime::testing::Location::new("src/ui/pages.ice", 545, 1, "rendered view node");
#[cfg(test)]
let __ice_render_source = ::ui_lang_runtime::testing::push_render_source(__ice_render_source_location);
let __ice_rendered: __IceElement<'_, __PagesViewMessage> = ::ui_lang_runtime::grow_stack(|| self.__ice_component_use_2(__ice_palette, format!("{}/Icon@1166", __for_scope)));
#[cfg(test)]
drop(__ice_render_source);
__ice_rendered
}); if (child.title).is_empty() { __children.push({
#[cfg(test)]
let __ice_render_source_location = ::ui_lang_runtime::testing::Location::new("src/ui/pages.ice", 551, 1, "rendered view node");
#[cfg(test)]
let __ice_render_source = ::ui_lang_runtime::testing::push_render_source(__ice_render_source_location);
let __ice_rendered: __IceElement<'_, __PagesViewMessage> = ::ui_lang_guest::wire::Node::Text { options: ::ui_lang_guest::wire::TextOptions { height: ::std::option::Option::None, align_y: ::std::option::Option::None, line_height: ::std::option::Option::None, shaping: ::std::option::Option::None, wrapping: ::std::option::Option::Some(::ui_lang_guest::wire::Wrapping::None), tracking: 0.0f32, font: ::std::option::Option::Some(::ui_lang_guest::wire::NamedFont { family: ::ui_lang_guest::wire::FontFamily::Named("Geist".into()), weight: ::ui_lang_guest::wire::Weight::Medium, stretch: ::ui_lang_guest::wire::FontStretch::Normal, style: ::ui_lang_guest::wire::FontStyle::Normal }) }, key: format!("{}/@text:551", __for_scope), size: ::std::option::Option::Some(((13.5) as f32).max(f32::EPSILON).min(f32::MAX)), color: ::std::option::Option::Some({ let __color: ::iced::Color = __ice_palette.colors[5]; ::ui_lang_guest::wire::Rgba([__color.r, __color.g, __color.b, __color.a]) }), font: ::ui_lang_guest::wire::Font { monospace: false, weight: ::ui_lang_guest::wire::Weight::Normal }, width: ::std::option::Option::Some(::ui_lang_guest::wire::Length::Fill), align_x: ::std::option::Option::None, content: ("Untitled".to_owned()).to_string() };
#[cfg(test)]
drop(__ice_render_source);
__ice_rendered
}); } if (!(child.title).is_empty()) { __children.push({
#[cfg(test)]
let __ice_render_source_location = ::ui_lang_runtime::testing::Location::new("src/ui/pages.ice", 559, 1, "rendered view node");
#[cfg(test)]
let __ice_render_source = ::ui_lang_runtime::testing::push_render_source(__ice_render_source_location);
let __ice_rendered: __IceElement<'_, __PagesViewMessage> = ::ui_lang_guest::wire::Node::Text { options: ::ui_lang_guest::wire::TextOptions { height: ::std::option::Option::None, align_y: ::std::option::Option::None, line_height: ::std::option::Option::None, shaping: ::std::option::Option::None, wrapping: ::std::option::Option::Some(::ui_lang_guest::wire::Wrapping::None), tracking: 0.0f32, font: ::std::option::Option::Some(::ui_lang_guest::wire::NamedFont { family: ::ui_lang_guest::wire::FontFamily::Named("Geist".into()), weight: ::ui_lang_guest::wire::Weight::Medium, stretch: ::ui_lang_guest::wire::FontStretch::Normal, style: ::ui_lang_guest::wire::FontStyle::Normal }) }, key: format!("{}/@text:559", __for_scope), size: ::std::option::Option::Some(((13.5) as f32).max(f32::EPSILON).min(f32::MAX)), color: ::std::option::Option::Some({ let __color: ::iced::Color = __ice_palette.colors[4]; ::ui_lang_guest::wire::Rgba([__color.r, __color.g, __color.b, __color.a]) }), font: ::ui_lang_guest::wire::Font { monospace: false, weight: ::ui_lang_guest::wire::Weight::Normal }, width: ::std::option::Option::Some(::ui_lang_guest::wire::Length::Fill), align_x: ::std::option::Option::None, content: (child.title.to_owned()).to_string() };
#[cfg(test)]
drop(__ice_render_source);
__ice_rendered
}); } __children.push({
#[cfg(test)]
let __ice_render_source_location = ::ui_lang_runtime::testing::Location::new("src/ui/pages.ice", 566, 1, "rendered view node");
#[cfg(test)]
let __ice_render_source = ::ui_lang_runtime::testing::push_render_source(__ice_render_source_location);
let __ice_rendered: __IceElement<'_, __PagesViewMessage> = ::ui_lang_guest::wire::Node::Text { options: ::ui_lang_guest::wire::TextOptions { height: ::std::option::Option::None, align_y: ::std::option::Option::None, line_height: ::std::option::Option::None, shaping: ::std::option::Option::None, wrapping: ::std::option::Option::Some(::ui_lang_guest::wire::Wrapping::None), tracking: 0.0f32, font: ::std::option::Option::Some(::ui_lang_guest::wire::NamedFont { family: ::ui_lang_guest::wire::FontFamily::Named("Geist".into()), weight: ::ui_lang_guest::wire::Weight::Normal, stretch: ::ui_lang_guest::wire::FontStretch::Normal, style: ::ui_lang_guest::wire::FontStyle::Normal }) }, key: format!("{}/@text:566", __for_scope), size: ::std::option::Option::Some(((13.0) as f32).max(f32::EPSILON).min(f32::MAX)), color: ::std::option::Option::Some({ let __color: ::iced::Color = __ice_palette.colors[73]; ::ui_lang_guest::wire::Rgba([__color.r, __color.g, __color.b, __color.a]) }), font: ::ui_lang_guest::wire::Font { monospace: false, weight: ::ui_lang_guest::wire::Weight::Normal }, width: ::std::option::Option::None, align_x: ::std::option::Option::None, content: ("›".to_owned()).to_string() };
#[cfg(test)]
drop(__ice_render_source);
__ice_rendered
}); ::ui_lang_guest::wire::Node::Linear { max_width: ::std::option::Option::None, clip: false, key: format!("{}/@layout:540", __for_scope), wrap: None, axis: ::ui_lang_guest::wire::Axis::Row, spacing: ::std::option::Option::Some((8.0) as f32), padding: ::std::option::Option::None, width: ::std::option::Option::Some(::ui_lang_guest::wire::Length::Fill), height: ::std::option::Option::None, align: ::std::option::Option::Some(::ui_lang_guest::wire::AlignX::Center), background: ::std::option::Option::None, border: ::std::option::Option::None, children: __children } };
#[cfg(test)]
drop(__ice_render_source);
__ice_rendered
})), label: ::std::option::Option::Some(::std::string::String::from("Open subpage".to_owned())), on_press: if ((!(self.host_error).is_empty())) { ::std::option::Option::None } else { ::std::option::Option::Some(::ui_lang_guest::slots::message((move |__event_0| __PagesViewMessage::ChoosePage(__event_0))(child.id.to_owned()))) }, width: ::std::option::Option::Some(::ui_lang_guest::wire::Length::Fill), height: ::std::option::Option::None, padding: ::std::option::Option::Some(::ui_lang_guest::wire::Edges::all((6.0) as f32)), style: ::ui_lang_guest::wire::ButtonStyle { preset: ::ui_lang_guest::wire::ButtonPreset::Primary, recipe: Some(::ui_lang_guest::wire::ButtonRecipe { base: ::ui_lang_guest::wire::Face { background: ::std::option::Option::Some({ let __color: ::iced::Color = ::iced::Color::from_rgba8(0, 0, 0, 0.000000); ::ui_lang_guest::wire::Rgba([__color.r, __color.g, __color.b, __color.a]) }), text: ::std::option::Option::Some({ let __color: ::iced::Color = __ice_palette.colors[4]; ::ui_lang_guest::wire::Rgba([__color.r, __color.g, __color.b, __color.a]) }), border: ::std::option::Option::Some(::ui_lang_guest::wire::Border { color: ::std::option::Option::None, width: ::std::option::Option::None, radius: ::std::option::Option::Some([8.0; 4]) }) }, hover_background: ::std::option::Option::Some({ let __color: ::iced::Color = __ice_palette.colors[14]; ::ui_lang_guest::wire::Rgba([__color.r, __color.g, __color.b, __color.a]) }), pressed_background: ::std::option::Option::Some({ let __color: ::iced::Color = __ice_palette.colors[39]; ::ui_lang_guest::wire::Rgba([__color.r, __color.g, __color.b, __color.a]) }), disabled_background: ::std::option::Option::None, disabled_text: ::std::option::Option::None, disabled_opacity: ::std::option::Option::Some(0.5f32), focus_ring: ::std::option::Option::Some({ let __color: ::iced::Color = __ice_palette.colors[42]; ::ui_lang_guest::wire::Rgba([__color.r, __color.g, __color.b, __color.a]) }), text_size: ::std::option::Option::Some(12.5f32), line_height: ::std::option::Option::None, font: Some(::ui_lang_guest::wire::NamedFont { family: ::ui_lang_guest::wire::FontFamily::Named("Geist".into()), weight: ::ui_lang_guest::wire::Weight::Semibold, stretch: ::ui_lang_guest::wire::FontStretch::Normal, style: ::ui_lang_guest::wire::FontStyle::Normal }) }), active: ::ui_lang_guest::wire::Face { background: ::std::option::Option::Some({ let __color: ::iced::Color = ::iced::Color::from_rgba8(0, 0, 0, 0.000000); ::ui_lang_guest::wire::Rgba([__color.r, __color.g, __color.b, __color.a]) }), text: ::std::option::Option::Some({ let __color: ::iced::Color = __ice_palette.colors[4]; ::ui_lang_guest::wire::Rgba([__color.r, __color.g, __color.b, __color.a]) }), border: ::std::option::Option::Some(::ui_lang_guest::wire::Border { color: ::std::option::Option::Some({ let __color: ::iced::Color = ::iced::Color::from_rgba8(0, 0, 0, 0.000000); ::ui_lang_guest::wire::Rgba([__color.r, __color.g, __color.b, __color.a]) }), width: ::std::option::Option::Some(((1.0) as f32).max(0.0).min(f32::MAX)), radius: ::std::option::Option::Some([((7.0) as f32).max(0.0).min(f32::MAX), ((7.0) as f32).max(0.0).min(f32::MAX), ((7.0) as f32).max(0.0).min(f32::MAX), ((7.0) as f32).max(0.0).min(f32::MAX)]) }) }, hovered: ::std::option::Option::Some(::ui_lang_guest::wire::Face { background: ::std::option::Option::Some({ let __color: ::iced::Color = { let mut __color = __ice_palette.colors[4]; __color.a = 0.040000; __color }; ::ui_lang_guest::wire::Rgba([__color.r, __color.g, __color.b, __color.a]) }), text: ::std::option::Option::Some({ let __color: ::iced::Color = __ice_palette.colors[4]; ::ui_lang_guest::wire::Rgba([__color.r, __color.g, __color.b, __color.a]) }), border: ::std::option::Option::Some(::ui_lang_guest::wire::Border { color: ::std::option::Option::Some({ let __color: ::iced::Color = { let mut __color = __ice_palette.colors[4]; __color.a = 0.070000; __color }; ::ui_lang_guest::wire::Rgba([__color.r, __color.g, __color.b, __color.a]) }), width: ::std::option::Option::None, radius: ::std::option::Option::None }) }), pressed: ::std::option::Option::Some(::ui_lang_guest::wire::Face { background: ::std::option::Option::Some({ let __color: ::iced::Color = { let mut __color = __ice_palette.colors[4]; __color.a = 0.080000; __color }; ::ui_lang_guest::wire::Rgba([__color.r, __color.g, __color.b, __color.a]) }), text: ::std::option::Option::Some({ let __color: ::iced::Color = __ice_palette.colors[4]; ::ui_lang_guest::wire::Rgba([__color.r, __color.g, __color.b, __color.a]) }), border: ::std::option::Option::None }), disabled: ::std::option::Option::None } };
#[cfg(test)]
drop(__ice_render_source);
__ice_rendered
}); } ::ui_lang_guest::wire::Node::Linear { max_width: ::std::option::Option::None, clip: false, key: format!("{}/@layout:519", __ice_use_scope), wrap: None, axis: ::ui_lang_guest::wire::Axis::Column, spacing: ::std::option::Option::Some((2.0) as f32), padding: ::std::option::Option::Some(::ui_lang_guest::wire::Edges { top: (10.0) as f32, right: (0.0) as f32, bottom: (0.0) as f32, left: (46.0) as f32 }), width: ::std::option::Option::Some(::ui_lang_guest::wire::Length::Fill), height: ::std::option::Option::None, align: ::std::option::Option::None, background: ::std::option::Option::None, border: ::std::option::Option::None, children: __children } };
#[cfg(test)]
drop(__ice_render_source);
__ice_rendered
}); } ::ui_lang_guest::wire::Node::Linear { max_width: ::std::option::Option::None, clip: false, key: format!("{}/@layout:445", __ice_use_scope), wrap: None, axis: ::ui_lang_guest::wire::Axis::Column, spacing: ::std::option::Option::Some((8.0) as f32), padding: ::std::option::Option::None, width: ::std::option::Option::Some(::ui_lang_guest::wire::Length::Fill), height: ::std::option::Option::Some(::ui_lang_guest::wire::Length::Fill), align: ::std::option::Option::None, background: ::std::option::Option::None, border: ::std::option::Option::None, children: __children } };
#[cfg(test)]
drop(__ice_render_source);
__ice_rendered
}) };
#[cfg(test)]
drop(__ice_render_source);
__ice_rendered
}); } if (self.connected && (!(self.page_search_hits).is_empty())) { __children.push({
#[cfg(test)]
let __ice_render_source_location = ::ui_lang_runtime::testing::Location::new("src/ui/pages.ice", 589, 1, "rendered view node");
#[cfg(test)]
let __ice_render_source = ::ui_lang_runtime::testing::push_render_source(__ice_render_source_location);
let __ice_rendered: __IceElement<'_, __PagesViewMessage> = ::ui_lang_guest::wire::Node::Container { shadow: ::ui_lang_guest::wire::Shadow { color: ::std::option::Option::None, x: ::std::option::Option::None, y: ::std::option::Option::None, blur: ::std::option::Option::None }, max_width: ::std::option::Option::None, max_height: ::std::option::Option::None, clip: false, key: format!("{}/@container:589", __ice_use_scope), width: ::std::option::Option::Some(::ui_lang_guest::wire::Length::Fill), height: ::std::option::Option::Some(::ui_lang_guest::wire::Length::Fill), padding: ::std::option::Option::Some(::ui_lang_guest::wire::Edges { top: (26.0) as f32, right: (40.0) as f32, bottom: (0.0) as f32, left: (22.0) as f32 }), align_x: ::std::option::Option::None, align_y: ::std::option::Option::Some(::ui_lang_guest::wire::AlignY::Top), background: (::std::option::Option::None).map(::ui_lang_guest::wire::Background::Color), border: ::std::option::Option::None, snap: ::std::option::Option::None, content: ::std::boxed::Box::new({
#[cfg(test)]
let __ice_render_source_location = ::ui_lang_runtime::testing::Location::new("src/ui/pages.ice", 597, 1, "rendered view node");
#[cfg(test)]
let __ice_render_source = ::ui_lang_runtime::testing::push_render_source(__ice_render_source_location);
let __ice_rendered: __IceElement<'_, __PagesViewMessage> = ::ui_lang_guest::wire::Node::Container { shadow: ::ui_lang_guest::wire::Shadow { color: ::std::option::Option::None, x: ::std::option::Option::None, y: ::std::option::Option::None, blur: ::std::option::Option::None }, max_width: ::std::option::Option::Some(((766.0) as f32).max(0.0).min(f32::MAX)), max_height: ::std::option::Option::None, clip: false, key: format!("{}/@container:597", __ice_use_scope), width: ::std::option::Option::Some(::ui_lang_guest::wire::Length::Fill), height: ::std::option::Option::Some(::ui_lang_guest::wire::Length::Fixed((148.0) as f32)), padding: ::std::option::Option::Some(::ui_lang_guest::wire::Edges { top: (5.0) as f32, right: (5.0) as f32, bottom: (5.0) as f32, left: (5.0) as f32 }), align_x: ::std::option::Option::None, align_y: ::std::option::Option::None, background: (::std::option::Option::Some({ let __color: ::iced::Color = __ice_palette.colors[55]; ::ui_lang_guest::wire::Rgba([__color.r, __color.g, __color.b, __color.a]) })).map(::ui_lang_guest::wire::Background::Color), border: ::std::option::Option::Some(::ui_lang_guest::wire::Border { color: ::std::option::Option::Some({ let __color: ::iced::Color = { let mut __color = __ice_palette.colors[4]; __color.a = 0.080000; __color }; ::ui_lang_guest::wire::Rgba([__color.r, __color.g, __color.b, __color.a]) }), width: ::std::option::Option::Some(((1.0) as f32).max(0.0).min(f32::MAX)), radius: ::std::option::Option::Some([((9.0) as f32).max(0.0).min(f32::MAX), ((9.0) as f32).max(0.0).min(f32::MAX), ((9.0) as f32).max(0.0).min(f32::MAX), ((9.0) as f32).max(0.0).min(f32::MAX)]) }), snap: ::std::option::Option::None, content: ::std::boxed::Box::new({
#[cfg(test)]
let __ice_render_source_location = ::ui_lang_runtime::testing::Location::new("src/ui/pages.ice", 607, 1, "rendered view node");
#[cfg(test)]
let __ice_render_source = ::ui_lang_runtime::testing::push_render_source(__ice_render_source_location);
let __ice_rendered: __IceElement<'_, __PagesViewMessage> = ::ui_lang_guest::wire::Node::Scroll { on_scroll: ::std::option::Option::None, virtual_rows: false, key: format!("{}/@layout:607", __ice_use_scope), direction: ::ui_lang_guest::wire::ScrollDirection::Vertical, width: ::std::option::Option::Some(::ui_lang_guest::wire::Length::Fill), height: ::std::option::Option::Some(::ui_lang_guest::wire::Length::Fill), bar_hidden: false, bar_width: ::std::option::Option::None, bar_margin: ::std::option::Option::None, scroller_width: ::std::option::Option::None, bar_spacing: ::std::option::Option::None, anchor_x: ::ui_lang_guest::wire::ScrollAnchor::Start, anchor_y: ::ui_lang_guest::wire::ScrollAnchor::Start, auto_scroll: (false), background: ::std::option::Option::None, border: ::std::option::Option::None, content: ::std::boxed::Box::new({
#[cfg(test)]
let __ice_render_source_location = ::ui_lang_runtime::testing::Location::new("src/ui/pages.ice", 612, 1, "rendered view node");
#[cfg(test)]
let __ice_render_source = ::ui_lang_runtime::testing::push_render_source(__ice_render_source_location);
let __ice_rendered: __IceElement<'_, __PagesViewMessage> = { let mut __children: ::std::vec::Vec<__IceElement<'_, __PagesViewMessage>> = ::std::vec::Vec::new(); for (__ice_index, hit) in self.page_search_hits.iter().enumerate() { let __for_scope = format!("{}/@for:1234({})", __ice_use_scope, __ice_index); __children.push({
#[cfg(test)]
let __ice_render_source_location = ::ui_lang_runtime::testing::Location::new("src/ui/pages.ice", 614, 1, "rendered view node");
#[cfg(test)]
let __ice_render_source = ::ui_lang_runtime::testing::push_render_source(__ice_render_source_location);
let __ice_rendered: __IceElement<'_, __PagesViewMessage> = ::ui_lang_runtime::grow_stack(|| self.__ice_component_use_9(__ice_palette, format!("{}/PageSearchResult@1235", __for_scope), (move |__event_0, __event_1| __PagesViewMessage::OpenPageSearchHit(__event_0, __event_1)).clone(), hit.clone()));
#[cfg(test)]
drop(__ice_render_source);
__ice_rendered
}); } ::ui_lang_guest::wire::Node::Linear { max_width: ::std::option::Option::None, clip: false, key: format!("{}/@layout:612", __ice_use_scope), wrap: None, axis: ::ui_lang_guest::wire::Axis::Column, spacing: ::std::option::Option::Some((1.0) as f32), padding: ::std::option::Option::None, width: ::std::option::Option::Some(::ui_lang_guest::wire::Length::Fill), height: ::std::option::Option::None, align: ::std::option::Option::None, background: ::std::option::Option::None, border: ::std::option::Option::None, children: __children } };
#[cfg(test)]
drop(__ice_render_source);
__ice_rendered
}) };
#[cfg(test)]
drop(__ice_render_source);
__ice_rendered
}) };
#[cfg(test)]
drop(__ice_render_source);
__ice_rendered
}) };
#[cfg(test)]
drop(__ice_render_source);
__ice_rendered
}); } if ((self.connected && (self.page_search_hits).is_empty()) && crate::host::search_answer_stands(::std::convert::AsRef::as_ref(&(self.page_search_query)), ::std::convert::AsRef::as_ref(&(self.page_search_draft)), self.page_searching)) { __children.push({
#[cfg(test)]
let __ice_render_source_location = ::ui_lang_runtime::testing::Location::new("src/ui/pages.ice", 641, 1, "rendered view node");
#[cfg(test)]
let __ice_render_source = ::ui_lang_runtime::testing::push_render_source(__ice_render_source_location);
let __ice_rendered: __IceElement<'_, __PagesViewMessage> = ::ui_lang_guest::wire::Node::Container { shadow: ::ui_lang_guest::wire::Shadow { color: ::std::option::Option::None, x: ::std::option::Option::None, y: ::std::option::Option::None, blur: ::std::option::Option::None }, max_width: ::std::option::Option::None, max_height: ::std::option::Option::None, clip: false, key: format!("{}/@container:641", __ice_use_scope), width: ::std::option::Option::Some(::ui_lang_guest::wire::Length::Fill), height: ::std::option::Option::Some(::ui_lang_guest::wire::Length::Fill), padding: ::std::option::Option::Some(::ui_lang_guest::wire::Edges { top: (26.0) as f32, right: (40.0) as f32, bottom: (0.0) as f32, left: (22.0) as f32 }), align_x: ::std::option::Option::None, align_y: ::std::option::Option::Some(::ui_lang_guest::wire::AlignY::Top), background: (::std::option::Option::None).map(::ui_lang_guest::wire::Background::Color), border: ::std::option::Option::None, snap: ::std::option::Option::None, content: ::std::boxed::Box::new({
#[cfg(test)]
let __ice_render_source_location = ::ui_lang_runtime::testing::Location::new("src/ui/pages.ice", 652, 1, "rendered view node");
#[cfg(test)]
let __ice_render_source = ::ui_lang_runtime::testing::push_render_source(__ice_render_source_location);
let __ice_rendered: __IceElement<'_, __PagesViewMessage> = ::ui_lang_guest::wire::Node::Container { shadow: ::ui_lang_guest::wire::Shadow { color: ::std::option::Option::Some({ let __color: ::iced::Color = __ice_palette.colors[46]; ::ui_lang_guest::wire::Rgba([__color.r, __color.g, __color.b, __color.a]) }), x: ::std::option::Option::None, y: ::std::option::Option::Some((8.0) as f32), blur: ::std::option::Option::Some((24.0) as f32) }, max_width: ::std::option::Option::Some(((766.0) as f32).max(0.0).min(f32::MAX)), max_height: ::std::option::Option::None, clip: false, key: format!("{}/@container:652", __ice_use_scope), width: ::std::option::Option::Some(::ui_lang_guest::wire::Length::Fill), height: ::std::option::Option::None, padding: ::std::option::Option::None, align_x: ::std::option::Option::None, align_y: ::std::option::Option::None, background: (::std::option::Option::Some({ let __color: ::iced::Color = __ice_palette.colors[55]; ::ui_lang_guest::wire::Rgba([__color.r, __color.g, __color.b, __color.a]) })).map(::ui_lang_guest::wire::Background::Color), border: ::std::option::Option::Some(::ui_lang_guest::wire::Border { color: ::std::option::Option::None, width: ::std::option::Option::None, radius: ::std::option::Option::Some([((12.0) as f32).max(0.0).min(f32::MAX), ((12.0) as f32).max(0.0).min(f32::MAX), ((12.0) as f32).max(0.0).min(f32::MAX), ((12.0) as f32).max(0.0).min(f32::MAX)]) }), snap: ::std::option::Option::None, content: ::std::boxed::Box::new({
#[cfg(test)]
let __ice_render_source_location = ::ui_lang_runtime::testing::Location::new("src/ui/pages.ice", 661, 1, "rendered view node");
#[cfg(test)]
let __ice_render_source = ::ui_lang_runtime::testing::push_render_source(__ice_render_source_location);
let __ice_rendered: __IceElement<'_, __PagesViewMessage> = ::ui_lang_runtime::grow_stack(|| self.__ice_component_use_10(__ice_palette, format!("{}/EmptyPlate@1282", __ice_use_scope)));
#[cfg(test)]
drop(__ice_render_source);
__ice_rendered
}) };
#[cfg(test)]
drop(__ice_render_source);
__ice_rendered
}) };
#[cfg(test)]
drop(__ice_render_source);
__ice_rendered
}); } if ((self.connected && (!(self.active_page).is_empty())) && self.block_comments_open) { __children.push({
#[cfg(test)]
let __ice_render_source_location = ::ui_lang_runtime::testing::Location::new("src/ui/pages.ice", 679, 1, "rendered view node");
#[cfg(test)]
let __ice_render_source = ::ui_lang_runtime::testing::push_render_source(__ice_render_source_location);
let __ice_rendered: __IceElement<'_, __PagesViewMessage> = ::ui_lang_guest::wire::Node::Sensor { key: format!("{}/@sensor:679", __ice_use_scope), reset: ::std::option::Option::None, on_show: ::std::option::Option::Some(::ui_lang_guest::slots::handler::<(f32, f32), __PagesViewMessage>(::std::boxed::Box::new({ let __route = {  { let __route_callback = (move |__event_0, __event_1| __PagesViewMessage::CommentsCardMeasured(__event_0, __event_1)).clone(); move |__size: (f64, f64)| (__route_callback)(__size.0, __size.1) } }; move |__sent: (f32, f32)| ::std::option::Option::Some(__route((f64::from(__sent.0), f64::from(__sent.1)))) }))), on_resize: ::std::option::Option::Some(::ui_lang_guest::slots::handler::<(f32, f32), __PagesViewMessage>(::std::boxed::Box::new({ let __route = {  { let __route_callback = (move |__event_0, __event_1| __PagesViewMessage::CommentsCardMeasured(__event_0, __event_1)).clone(); move |__size: (f64, f64)| (__route_callback)(__size.0, __size.1) } }; move |__sent: (f32, f32)| ::std::option::Option::Some(__route((f64::from(__sent.0), f64::from(__sent.1)))) }))), on_hide: ::std::option::Option::None, anticipate: ::std::option::Option::None, delay: ::std::option::Option::None, child: ::std::boxed::Box::new({
#[cfg(test)]
let __ice_render_source_location = ::ui_lang_runtime::testing::Location::new("src/ui/pages.ice", 680, 1, "rendered view node");
#[cfg(test)]
let __ice_render_source = ::ui_lang_runtime::testing::push_render_source(__ice_render_source_location);
let __ice_rendered: __IceElement<'_, __PagesViewMessage> = ::ui_lang_guest::wire::Node::Float { key: format!("{}/@float:680", __ice_use_scope), x: ::ui_lang_guest::wire::FloatExpression { ops: vec![::ui_lang_guest::wire::FloatOp::Geometry(4),::ui_lang_guest::wire::FloatOp::Geometry(6),::ui_lang_guest::wire::FloatOp::Add,::ui_lang_guest::wire::FloatOp::Geometry(0),::ui_lang_guest::wire::FloatOp::Subtract,::ui_lang_guest::wire::FloatOp::Geometry(2),::ui_lang_guest::wire::FloatOp::Subtract,::ui_lang_guest::wire::FloatOp::Number((crate::host::comments_right_anchor(self.pages_pane_width)) as f64),::ui_lang_guest::wire::FloatOp::Multiply,::ui_lang_guest::wire::FloatOp::Number((crate::host::comments_left_inset(self.pages_pane_width)) as f64),::ui_lang_guest::wire::FloatOp::Add] }, y: ::ui_lang_guest::wire::FloatExpression { ops: vec![::ui_lang_guest::wire::FloatOp::Number((crate::host::comment_card_offset(self.pages_pane_width, self.comment_anchor_y, self.pages_viewport_height)) as f64)] }, scale: ((1.0) as f32).max(f32::EPSILON).min(f32::MAX), shadow: ::ui_lang_guest::wire::Shadow { color: ::std::option::Option::None, x: ::std::option::Option::None, y: ::std::option::Option::None, blur: ::std::option::Option::None }, radius: ::std::option::Option::None, content: Box::new({
#[cfg(test)]
let __ice_render_source_location = ::ui_lang_runtime::testing::Location::new("src/ui/pages.ice", 681, 1, "rendered view node");
#[cfg(test)]
let __ice_render_source = ::ui_lang_runtime::testing::push_render_source(__ice_render_source_location);
let __ice_rendered: __IceElement<'_, __PagesViewMessage> = { let __ice_node_scope = format!("{}/comments-card", __ice_use_scope); ::ui_lang_guest::wire::Node::Container { shadow: ::ui_lang_guest::wire::Shadow { color: ::std::option::Option::Some({ let __color: ::iced::Color = __ice_palette.colors[46]; ::ui_lang_guest::wire::Rgba([__color.r, __color.g, __color.b, __color.a]) }), x: ::std::option::Option::None, y: ::std::option::Option::Some((8.0) as f32), blur: ::std::option::Option::Some((24.0) as f32) }, max_width: ::std::option::Option::None, max_height: ::std::option::Option::Some(((crate::host::comment_card_height(self.comment_anchor_y, self.pages_viewport_height)) as f32).max(0.0).min(f32::MAX)), clip: true, key: __ice_node_scope.clone(), width: ::std::option::Option::Some(::ui_lang_guest::wire::Length::Fixed((crate::host::comments_card_width(self.pages_pane_width)) as f32)), height: ::std::option::Option::Some(::ui_lang_guest::wire::Length::Shrink), padding: ::std::option::Option::None, align_x: ::std::option::Option::None, align_y: ::std::option::Option::None, background: (::std::option::Option::Some({ let __color: ::iced::Color = __ice_palette.colors[55]; ::ui_lang_guest::wire::Rgba([__color.r, __color.g, __color.b, __color.a]) })).map(::ui_lang_guest::wire::Background::Color), border: ::std::option::Option::Some(::ui_lang_guest::wire::Border { color: ::std::option::Option::Some({ let __color: ::iced::Color = __ice_palette.colors[60]; ::ui_lang_guest::wire::Rgba([__color.r, __color.g, __color.b, __color.a]) }), width: ::std::option::Option::Some(((1.0) as f32).max(0.0).min(f32::MAX)), radius: ::std::option::Option::Some([((12.0) as f32).max(0.0).min(f32::MAX), ((12.0) as f32).max(0.0).min(f32::MAX), ((12.0) as f32).max(0.0).min(f32::MAX), ((12.0) as f32).max(0.0).min(f32::MAX)]) }), snap: ::std::option::Option::None, content: ::std::boxed::Box::new({
#[cfg(test)]
let __ice_render_source_location = ::ui_lang_runtime::testing::Location::new("src/ui/pages.ice", 694, 1, "rendered view node");
#[cfg(test)]
let __ice_render_source = ::ui_lang_runtime::testing::push_render_source(__ice_render_source_location);
let __ice_rendered: __IceElement<'_, __PagesViewMessage> = { let mut __children: ::std::vec::Vec<__IceElement<'_, __PagesViewMessage>> = ::std::vec::Vec::new(); __children.push({
#[cfg(test)]
let __ice_render_source_location = ::ui_lang_runtime::testing::Location::new("src/ui/pages.ice", 706, 1, "rendered view node");
#[cfg(test)]
let __ice_render_source = ::ui_lang_runtime::testing::push_render_source(__ice_render_source_location);
let __ice_rendered: __IceElement<'_, __PagesViewMessage> = ::ui_lang_guest::wire::Node::Container { shadow: ::ui_lang_guest::wire::Shadow { color: ::std::option::Option::None, x: ::std::option::Option::None, y: ::std::option::Option::None, blur: ::std::option::Option::None }, max_width: ::std::option::Option::None, max_height: ::std::option::Option::None, clip: false, key: format!("{}/@container:706", __ice_use_scope), width: ::std::option::Option::Some(::ui_lang_guest::wire::Length::Fill), height: ::std::option::Option::Some(::ui_lang_guest::wire::Length::Fixed((44.0) as f32)), padding: ::std::option::Option::Some(::ui_lang_guest::wire::Edges { top: (0.0) as f32, right: (16.0) as f32, bottom: (0.0) as f32, left: (16.0) as f32 }), align_x: ::std::option::Option::None, align_y: ::std::option::Option::None, background: (::std::option::Option::None).map(::ui_lang_guest::wire::Background::Color), border: ::std::option::Option::None, snap: ::std::option::Option::None, content: ::std::boxed::Box::new({
#[cfg(test)]
let __ice_render_source_location = ::ui_lang_runtime::testing::Location::new("src/ui/pages.ice", 712, 1, "rendered view node");
#[cfg(test)]
let __ice_render_source = ::ui_lang_runtime::testing::push_render_source(__ice_render_source_location);
let __ice_rendered: __IceElement<'_, __PagesViewMessage> = { let mut __children: ::std::vec::Vec<__IceElement<'_, __PagesViewMessage>> = ::std::vec::Vec::new(); __children.push({
#[cfg(test)]
let __ice_render_source_location = ::ui_lang_runtime::testing::Location::new("src/ui/pages.ice", 718, 1, "rendered view node");
#[cfg(test)]
let __ice_render_source = ::ui_lang_runtime::testing::push_render_source(__ice_render_source_location);
let __ice_rendered: __IceElement<'_, __PagesViewMessage> = ::ui_lang_guest::wire::Node::Container { shadow: ::ui_lang_guest::wire::Shadow { color: ::std::option::Option::None, x: ::std::option::Option::None, y: ::std::option::Option::None, blur: ::std::option::Option::None }, max_width: ::std::option::Option::None, max_height: ::std::option::Option::None, clip: true, key: format!("{}/@container:718", __ice_use_scope), width: ::std::option::Option::Some(::ui_lang_guest::wire::Length::Fill), height: ::std::option::Option::None, padding: ::std::option::Option::None, align_x: ::std::option::Option::None, align_y: ::std::option::Option::None, background: (::std::option::Option::None).map(::ui_lang_guest::wire::Background::Color), border: ::std::option::Option::None, snap: ::std::option::Option::None, content: ::std::boxed::Box::new({
#[cfg(test)]
let __ice_render_source_location = ::ui_lang_runtime::testing::Location::new("src/ui/pages.ice", 719, 1, "rendered view node");
#[cfg(test)]
let __ice_render_source = ::ui_lang_runtime::testing::push_render_source(__ice_render_source_location);
let __ice_rendered: __IceElement<'_, __PagesViewMessage> = ::ui_lang_guest::wire::Node::Text { options: ::ui_lang_guest::wire::TextOptions { height: ::std::option::Option::None, align_y: ::std::option::Option::None, line_height: ::std::option::Option::None, shaping: ::std::option::Option::None, wrapping: ::std::option::Option::Some(::ui_lang_guest::wire::Wrapping::None), tracking: 0.0f32, font: ::std::option::Option::Some(::ui_lang_guest::wire::NamedFont { family: ::ui_lang_guest::wire::FontFamily::Named("Geist".into()), weight: ::ui_lang_guest::wire::Weight::Normal, stretch: ::ui_lang_guest::wire::FontStretch::Normal, style: ::ui_lang_guest::wire::FontStyle::Normal }) }, key: format!("{}/@text:719", __ice_use_scope), size: ::std::option::Option::Some(((13.0) as f32).max(f32::EPSILON).min(f32::MAX)), color: ::std::option::Option::Some({ let __color: ::iced::Color = __ice_palette.colors[4]; ::ui_lang_guest::wire::Rgba([__color.r, __color.g, __color.b, __color.a]) }), font: ::ui_lang_guest::wire::Font { monospace: false, weight: ::ui_lang_guest::wire::Weight::Normal }, width: ::std::option::Option::None, align_x: ::std::option::Option::None, content: (crate::host::comment_scope_label(::std::convert::AsRef::as_ref(&(self.blocks)), ::std::convert::AsRef::as_ref(&(self.scope_target)), ::std::convert::AsRef::as_ref(&(self.active_page)), self.thread_total).to_owned()).to_string() };
#[cfg(test)]
drop(__ice_render_source);
__ice_rendered
}) };
#[cfg(test)]
drop(__ice_render_source);
__ice_rendered
}); if ((!self.scope_pinned) && (!(self.scope_target).is_empty())) { __children.push({
#[cfg(test)]
let __ice_render_source_location = ::ui_lang_runtime::testing::Location::new("src/ui/pages.ice", 725, 1, "rendered view node");
#[cfg(test)]
let __ice_render_source = ::ui_lang_runtime::testing::push_render_source(__ice_render_source_location);
let __ice_rendered: __IceElement<'_, __PagesViewMessage> = ::ui_lang_guest::wire::Node::Button { checked: ::std::option::Option::None, expanded: ::std::option::Option::None, description: ::std::option::Option::None, key: format!("{}/@button:725", __ice_use_scope), content: ::ui_lang_guest::wire::ButtonContent::Label(::std::string::String::from("← This page")), label: ::std::option::Option::Some(::std::string::String::from("All comments on this page".to_owned())), on_press: if ((self.busy || (!(self.host_error).is_empty()))) { ::std::option::Option::None } else { ::std::option::Option::Some(::ui_lang_guest::slots::message((move || __PagesViewMessage::WidenCommentScope)())) }, width: ::std::option::Option::None, height: ::std::option::Option::None, padding: ::std::option::Option::Some(::ui_lang_guest::wire::Edges::all((4.0) as f32)), style: ::ui_lang_guest::wire::ButtonStyle { preset: ::ui_lang_guest::wire::ButtonPreset::Primary, recipe: Some(::ui_lang_guest::wire::ButtonRecipe { base: ::ui_lang_guest::wire::Face { background: ::std::option::Option::Some({ let __color: ::iced::Color = __ice_palette.colors[12]; ::ui_lang_guest::wire::Rgba([__color.r, __color.g, __color.b, __color.a]) }), text: ::std::option::Option::Some({ let __color: ::iced::Color = __ice_palette.colors[13]; ::ui_lang_guest::wire::Rgba([__color.r, __color.g, __color.b, __color.a]) }), border: ::std::option::Option::Some(::ui_lang_guest::wire::Border { color: ::std::option::Option::Some({ let __color: ::iced::Color = __ice_palette.colors[40]; ::ui_lang_guest::wire::Rgba([__color.r, __color.g, __color.b, __color.a]) }), width: ::std::option::Option::Some(1.0), radius: ::std::option::Option::Some([9.0; 4]) }) }, hover_background: ::std::option::Option::Some({ let __color: ::iced::Color = __ice_palette.colors[14]; ::ui_lang_guest::wire::Rgba([__color.r, __color.g, __color.b, __color.a]) }), pressed_background: ::std::option::Option::Some({ let __color: ::iced::Color = __ice_palette.colors[6]; ::ui_lang_guest::wire::Rgba([__color.r, __color.g, __color.b, __color.a]) }), disabled_background: ::std::option::Option::None, disabled_text: ::std::option::Option::None, disabled_opacity: ::std::option::Option::Some(0.5f32), focus_ring: ::std::option::Option::Some({ let __color: ::iced::Color = __ice_palette.colors[42]; ::ui_lang_guest::wire::Rgba([__color.r, __color.g, __color.b, __color.a]) }), text_size: ::std::option::Option::Some(11.0f32), line_height: ::std::option::Option::Some(1.35f32), font: Some(::ui_lang_guest::wire::NamedFont { family: ::ui_lang_guest::wire::FontFamily::Named("Geist".into()), weight: ::ui_lang_guest::wire::Weight::Medium, stretch: ::ui_lang_guest::wire::FontStretch::Normal, style: ::ui_lang_guest::wire::FontStyle::Normal }) }), active: ::ui_lang_guest::wire::Face { background: ::std::option::Option::Some({ let __color: ::iced::Color = ::iced::Color::from_rgba8(0, 0, 0, 0.000000); ::ui_lang_guest::wire::Rgba([__color.r, __color.g, __color.b, __color.a]) }), text: ::std::option::Option::Some({ let __color: ::iced::Color = __ice_palette.colors[5]; ::ui_lang_guest::wire::Rgba([__color.r, __color.g, __color.b, __color.a]) }), border: ::std::option::Option::Some(::ui_lang_guest::wire::Border { color: ::std::option::Option::None, width: ::std::option::Option::None, radius: ::std::option::Option::Some([((6.0) as f32).max(0.0).min(f32::MAX), ((6.0) as f32).max(0.0).min(f32::MAX), ((6.0) as f32).max(0.0).min(f32::MAX), ((6.0) as f32).max(0.0).min(f32::MAX)]) }) }, hovered: ::std::option::Option::Some(::ui_lang_guest::wire::Face { background: ::std::option::Option::Some({ let __color: ::iced::Color = { let mut __color = __ice_palette.colors[4]; __color.a = 0.090000; __color }; ::ui_lang_guest::wire::Rgba([__color.r, __color.g, __color.b, __color.a]) }), text: ::std::option::Option::Some({ let __color: ::iced::Color = __ice_palette.colors[4]; ::ui_lang_guest::wire::Rgba([__color.r, __color.g, __color.b, __color.a]) }), border: ::std::option::Option::None }), pressed: ::std::option::Option::Some(::ui_lang_guest::wire::Face { background: ::std::option::Option::Some({ let __color: ::iced::Color = { let mut __color = __ice_palette.colors[4]; __color.a = 0.140000; __color }; ::ui_lang_guest::wire::Rgba([__color.r, __color.g, __color.b, __color.a]) }), text: ::std::option::Option::None, border: ::std::option::Option::None }), disabled: ::std::option::Option::None } };
#[cfg(test)]
drop(__ice_render_source);
__ice_rendered
}); } __children.push({
#[cfg(test)]
let __ice_render_source_location = ::ui_lang_runtime::testing::Location::new("src/ui/pages.ice", 734, 1, "rendered view node");
#[cfg(test)]
let __ice_render_source = ::ui_lang_runtime::testing::push_render_source(__ice_render_source_location);
let __ice_rendered: __IceElement<'_, __PagesViewMessage> = ::ui_lang_guest::wire::Node::Button { checked: ::std::option::Option::None, expanded: ::std::option::Option::None, description: ::std::option::Option::None, key: format!("{}/@button:734", __ice_use_scope), content: ::ui_lang_guest::wire::ButtonContent::Child(::std::boxed::Box::new({
#[cfg(test)]
let __ice_render_source_location = ::ui_lang_runtime::testing::Location::new("src/ui/pages.ice", 742, 1, "rendered view node");
#[cfg(test)]
let __ice_render_source = ::ui_lang_runtime::testing::push_render_source(__ice_render_source_location);
let __ice_rendered: __IceElement<'_, __PagesViewMessage> = ::ui_lang_guest::wire::Node::Container { shadow: ::ui_lang_guest::wire::Shadow { color: ::std::option::Option::None, x: ::std::option::Option::None, y: ::std::option::Option::None, blur: ::std::option::Option::None }, max_width: ::std::option::Option::None, max_height: ::std::option::Option::None, clip: false, key: format!("{}/@container:742", __ice_use_scope), width: ::std::option::Option::Some(::ui_lang_guest::wire::Length::Fill), height: ::std::option::Option::Some(::ui_lang_guest::wire::Length::Fill), padding: ::std::option::Option::None, align_x: ::std::option::Option::Some(::ui_lang_guest::wire::AlignX::Center), align_y: ::std::option::Option::Some(::ui_lang_guest::wire::AlignY::Center), background: (::std::option::Option::None).map(::ui_lang_guest::wire::Background::Color), border: ::std::option::Option::None, snap: ::std::option::Option::None, content: ::std::boxed::Box::new({
#[cfg(test)]
let __ice_render_source_location = ::ui_lang_runtime::testing::Location::new("src/ui/pages.ice", 748, 1, "rendered view node");
#[cfg(test)]
let __ice_render_source = ::ui_lang_runtime::testing::push_render_source(__ice_render_source_location);
let __ice_rendered: __IceElement<'_, __PagesViewMessage> = ::ui_lang_guest::wire::Node::Text { options: ::ui_lang_guest::wire::TextOptions { height: ::std::option::Option::None, align_y: ::std::option::Option::None, line_height: ::std::option::Option::None, shaping: ::std::option::Option::None, wrapping: ::std::option::Option::Some(::ui_lang_guest::wire::Wrapping::None), tracking: 0.0f32, font: ::std::option::Option::Some(::ui_lang_guest::wire::NamedFont { family: ::ui_lang_guest::wire::FontFamily::Named("Geist".into()), weight: ::ui_lang_guest::wire::Weight::Normal, stretch: ::ui_lang_guest::wire::FontStretch::Normal, style: ::ui_lang_guest::wire::FontStyle::Normal }) }, key: format!("{}/@text:748", __ice_use_scope), size: ::std::option::Option::Some(((13.0) as f32).max(f32::EPSILON).min(f32::MAX)), color: ::std::option::Option::Some({ let __color: ::iced::Color = __ice_palette.colors[5]; ::ui_lang_guest::wire::Rgba([__color.r, __color.g, __color.b, __color.a]) }), font: ::ui_lang_guest::wire::Font { monospace: false, weight: ::ui_lang_guest::wire::Weight::Normal }, width: ::std::option::Option::None, align_x: ::std::option::Option::None, content: ("×".to_owned()).to_string() };
#[cfg(test)]
drop(__ice_render_source);
__ice_rendered
}) };
#[cfg(test)]
drop(__ice_render_source);
__ice_rendered
})), label: ::std::option::Option::Some(::std::string::String::from("Close comments".to_owned())), on_press: if ((self.busy || (!(self.host_error).is_empty()))) { ::std::option::Option::None } else { ::std::option::Option::Some(::ui_lang_guest::slots::message((move || __PagesViewMessage::CloseBlockComments)())) }, width: ::std::option::Option::Some(::ui_lang_guest::wire::Length::Fixed((24.0) as f32)), height: ::std::option::Option::Some(::ui_lang_guest::wire::Length::Fixed((24.0) as f32)), padding: ::std::option::Option::Some(::ui_lang_guest::wire::Edges::all((4.0) as f32)), style: ::ui_lang_guest::wire::ButtonStyle { preset: ::ui_lang_guest::wire::ButtonPreset::Primary, recipe: Some(::ui_lang_guest::wire::ButtonRecipe { base: ::ui_lang_guest::wire::Face { background: ::std::option::Option::Some({ let __color: ::iced::Color = ::iced::Color::from_rgba8(0, 0, 0, 0.000000); ::ui_lang_guest::wire::Rgba([__color.r, __color.g, __color.b, __color.a]) }), text: ::std::option::Option::Some({ let __color: ::iced::Color = __ice_palette.colors[4]; ::ui_lang_guest::wire::Rgba([__color.r, __color.g, __color.b, __color.a]) }), border: ::std::option::Option::Some(::ui_lang_guest::wire::Border { color: ::std::option::Option::None, width: ::std::option::Option::None, radius: ::std::option::Option::Some([7.0; 4]) }) }, hover_background: ::std::option::Option::Some({ let __color: ::iced::Color = __ice_palette.colors[14]; ::ui_lang_guest::wire::Rgba([__color.r, __color.g, __color.b, __color.a]) }), pressed_background: ::std::option::Option::Some({ let __color: ::iced::Color = __ice_palette.colors[39]; ::ui_lang_guest::wire::Rgba([__color.r, __color.g, __color.b, __color.a]) }), disabled_background: ::std::option::Option::None, disabled_text: ::std::option::Option::None, disabled_opacity: ::std::option::Option::Some(0.5f32), focus_ring: ::std::option::Option::Some({ let __color: ::iced::Color = __ice_palette.colors[42]; ::ui_lang_guest::wire::Rgba([__color.r, __color.g, __color.b, __color.a]) }), text_size: ::std::option::Option::Some(13.5f32), line_height: ::std::option::Option::None, font: Some(::ui_lang_guest::wire::NamedFont { family: ::ui_lang_guest::wire::FontFamily::Named("Geist".into()), weight: ::ui_lang_guest::wire::Weight::Normal, stretch: ::ui_lang_guest::wire::FontStretch::Normal, style: ::ui_lang_guest::wire::FontStyle::Normal }) }), active: ::ui_lang_guest::wire::Face { background: ::std::option::Option::Some({ let __color: ::iced::Color = ::iced::Color::from_rgba8(0, 0, 0, 0.000000); ::ui_lang_guest::wire::Rgba([__color.r, __color.g, __color.b, __color.a]) }), text: ::std::option::Option::Some({ let __color: ::iced::Color = __ice_palette.colors[5]; ::ui_lang_guest::wire::Rgba([__color.r, __color.g, __color.b, __color.a]) }), border: ::std::option::Option::Some(::ui_lang_guest::wire::Border { color: ::std::option::Option::Some({ let __color: ::iced::Color = ::iced::Color::from_rgba8(0, 0, 0, 0.000000); ::ui_lang_guest::wire::Rgba([__color.r, __color.g, __color.b, __color.a]) }), width: ::std::option::Option::Some(((1.0) as f32).max(0.0).min(f32::MAX)), radius: ::std::option::Option::Some([((6.0) as f32).max(0.0).min(f32::MAX), ((6.0) as f32).max(0.0).min(f32::MAX), ((6.0) as f32).max(0.0).min(f32::MAX), ((6.0) as f32).max(0.0).min(f32::MAX)]) }) }, hovered: ::std::option::Option::Some(::ui_lang_guest::wire::Face { background: ::std::option::Option::Some({ let __color: ::iced::Color = __ice_palette.colors[56]; ::ui_lang_guest::wire::Rgba([__color.r, __color.g, __color.b, __color.a]) }), text: ::std::option::Option::Some({ let __color: ::iced::Color = __ice_palette.colors[4]; ::ui_lang_guest::wire::Rgba([__color.r, __color.g, __color.b, __color.a]) }), border: ::std::option::Option::None }), pressed: ::std::option::Option::Some(::ui_lang_guest::wire::Face { background: ::std::option::Option::Some({ let __color: ::iced::Color = __ice_palette.colors[60]; ::ui_lang_guest::wire::Rgba([__color.r, __color.g, __color.b, __color.a]) }), text: ::std::option::Option::Some({ let __color: ::iced::Color = __ice_palette.colors[4]; ::ui_lang_guest::wire::Rgba([__color.r, __color.g, __color.b, __color.a]) }), border: ::std::option::Option::None }), disabled: ::std::option::Option::None } };
#[cfg(test)]
drop(__ice_render_source);
__ice_rendered
}); ::ui_lang_guest::wire::Node::Linear { max_width: ::std::option::Option::None, clip: false, key: format!("{}/@layout:712", __ice_use_scope), wrap: None, axis: ::ui_lang_guest::wire::Axis::Row, spacing: ::std::option::Option::Some((8.0) as f32), padding: ::std::option::Option::None, width: ::std::option::Option::Some(::ui_lang_guest::wire::Length::Fill), height: ::std::option::Option::Some(::ui_lang_guest::wire::Length::Fill), align: ::std::option::Option::Some(::ui_lang_guest::wire::AlignX::Center), background: ::std::option::Option::None, border: ::std::option::Option::None, children: __children } };
#[cfg(test)]
drop(__ice_render_source);
__ice_rendered
}) };
#[cfg(test)]
drop(__ice_render_source);
__ice_rendered
}); __children.push({
#[cfg(test)]
let __ice_render_source_location = ::ui_lang_runtime::testing::Location::new("src/ui/pages.ice", 756, 1, "rendered view node");
#[cfg(test)]
let __ice_render_source = ::ui_lang_runtime::testing::push_render_source(__ice_render_source_location);
let __ice_rendered: __IceElement<'_, __PagesViewMessage> = ::ui_lang_guest::wire::Node::Container { shadow: ::ui_lang_guest::wire::Shadow { color: ::std::option::Option::None, x: ::std::option::Option::None, y: ::std::option::Option::None, blur: ::std::option::Option::None }, max_width: ::std::option::Option::None, max_height: ::std::option::Option::None, clip: false, key: format!("{}/@container:756", __ice_use_scope), width: ::std::option::Option::Some(::ui_lang_guest::wire::Length::Fill), height: ::std::option::Option::Some(::ui_lang_guest::wire::Length::Fixed((1.0) as f32)), padding: ::std::option::Option::None, align_x: ::std::option::Option::None, align_y: ::std::option::Option::None, background: (::std::option::Option::Some({ let __color: ::iced::Color = __ice_palette.colors[60]; ::ui_lang_guest::wire::Rgba([__color.r, __color.g, __color.b, __color.a]) })).map(::ui_lang_guest::wire::Background::Color), border: ::std::option::Option::None, snap: ::std::option::Option::None, content: ::std::boxed::Box::new({
#[cfg(test)]
let __ice_render_source_location = ::ui_lang_runtime::testing::Location::new("src/ui/pages.ice", 761, 1, "rendered view node");
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
}); __children.push({
#[cfg(test)]
let __ice_render_source_location = ::ui_lang_runtime::testing::Location::new("src/ui/pages.ice", 762, 1, "rendered view node");
#[cfg(test)]
let __ice_render_source = ::ui_lang_runtime::testing::push_render_source(__ice_render_source_location);
let __ice_rendered: __IceElement<'_, __PagesViewMessage> = { let mut __children: ::std::vec::Vec<__IceElement<'_, __PagesViewMessage>> = ::std::vec::Vec::new(); if self.threads_loading { __children.push({
#[cfg(test)]
let __ice_render_source_location = ::ui_lang_runtime::testing::Location::new("src/ui/pages.ice", 769, 1, "rendered view node");
#[cfg(test)]
let __ice_render_source = ::ui_lang_runtime::testing::push_render_source(__ice_render_source_location);
let __ice_rendered: __IceElement<'_, __PagesViewMessage> = ::ui_lang_guest::wire::Node::Text { options: ::ui_lang_guest::wire::TextOptions { height: ::std::option::Option::None, align_y: ::std::option::Option::None, line_height: ::std::option::Option::None, shaping: ::std::option::Option::None, wrapping: ::std::option::Option::None, tracking: 0.0f32, font: ::std::option::Option::Some(::ui_lang_guest::wire::NamedFont { family: ::ui_lang_guest::wire::FontFamily::Named("Geist".into()), weight: ::ui_lang_guest::wire::Weight::Normal, stretch: ::ui_lang_guest::wire::FontStretch::Normal, style: ::ui_lang_guest::wire::FontStyle::Normal }) }, key: format!("{}/@text:769", __ice_use_scope), size: ::std::option::Option::Some(((12.5) as f32).max(f32::EPSILON).min(f32::MAX)), color: ::std::option::Option::Some({ let __color: ::iced::Color = __ice_palette.colors[5]; ::ui_lang_guest::wire::Rgba([__color.r, __color.g, __color.b, __color.a]) }), font: ::ui_lang_guest::wire::Font { monospace: false, weight: ::ui_lang_guest::wire::Weight::Normal }, width: ::std::option::Option::None, align_x: ::std::option::Option::None, content: ("Loading comments…".to_owned()).to_string() };
#[cfg(test)]
drop(__ice_render_source);
__ice_rendered
}); } __children.push({
#[cfg(test)]
let __ice_render_source_location = ::ui_lang_runtime::testing::Location::new("src/ui/pages.ice", 774, 1, "rendered view node");
#[cfg(test)]
let __ice_render_source = ::ui_lang_runtime::testing::push_render_source(__ice_render_source_location);
let __ice_rendered: __IceElement<'_, __PagesViewMessage> = ::ui_lang_guest::wire::Node::Container { shadow: ::ui_lang_guest::wire::Shadow { color: ::std::option::Option::None, x: ::std::option::Option::None, y: ::std::option::Option::None, blur: ::std::option::Option::None }, max_width: ::std::option::Option::None, max_height: ::std::option::Option::Some((((crate::host::comment_card_height(self.comment_anchor_y, self.pages_viewport_height) - 150.0)) as f32).max(0.0).min(f32::MAX)), clip: false, key: format!("{}/@container:774", __ice_use_scope), width: ::std::option::Option::Some(::ui_lang_guest::wire::Length::Fill), height: ::std::option::Option::Some(::ui_lang_guest::wire::Length::Shrink), padding: ::std::option::Option::None, align_x: ::std::option::Option::None, align_y: ::std::option::Option::None, background: (::std::option::Option::None).map(::ui_lang_guest::wire::Background::Color), border: ::std::option::Option::None, snap: ::std::option::Option::None, content: ::std::boxed::Box::new({
#[cfg(test)]
let __ice_render_source_location = ::ui_lang_runtime::testing::Location::new("src/ui/pages.ice", 775, 1, "rendered view node");
#[cfg(test)]
let __ice_render_source = ::ui_lang_runtime::testing::push_render_source(__ice_render_source_location);
let __ice_rendered: __IceElement<'_, __PagesViewMessage> = ::ui_lang_guest::wire::Node::Scroll { on_scroll: ::std::option::Option::None, virtual_rows: false, key: format!("{}/@layout:775", __ice_use_scope), direction: ::ui_lang_guest::wire::ScrollDirection::Vertical, width: ::std::option::Option::Some(::ui_lang_guest::wire::Length::Fill), height: ::std::option::Option::Some(::ui_lang_guest::wire::Length::Shrink), bar_hidden: false, bar_width: ::std::option::Option::None, bar_margin: ::std::option::Option::None, scroller_width: ::std::option::Option::None, bar_spacing: ::std::option::Option::None, anchor_x: ::ui_lang_guest::wire::ScrollAnchor::Start, anchor_y: ::ui_lang_guest::wire::ScrollAnchor::Start, auto_scroll: (false), background: ::std::option::Option::None, border: ::std::option::Option::None, content: ::std::boxed::Box::new({
#[cfg(test)]
let __ice_render_source_location = ::ui_lang_runtime::testing::Location::new("src/ui/pages.ice", 784, 1, "rendered view node");
#[cfg(test)]
let __ice_render_source = ::ui_lang_runtime::testing::push_render_source(__ice_render_source_location);
let __ice_rendered: __IceElement<'_, __PagesViewMessage> = { let mut __children: ::std::vec::Vec<__IceElement<'_, __PagesViewMessage>> = ::std::vec::Vec::new(); if (((crate::host::scope_groups(self.comment_rows.clone(), ::std::convert::AsRef::as_ref(&(self.scope_target)), ::std::convert::AsRef::as_ref(&(self.active_page)))).is_empty() && (crate::host::scope_resolved(self.comment_rows.clone(), ::std::convert::AsRef::as_ref(&(self.scope_target)))).is_empty()) && (!self.threads_loading)) { __children.push({
#[cfg(test)]
let __ice_render_source_location = ::ui_lang_runtime::testing::Location::new("src/ui/pages.ice", 786, 1, "rendered view node");
#[cfg(test)]
let __ice_render_source = ::ui_lang_runtime::testing::push_render_source(__ice_render_source_location);
let __ice_rendered: __IceElement<'_, __PagesViewMessage> = ::ui_lang_guest::wire::Node::Text { options: ::ui_lang_guest::wire::TextOptions { height: ::std::option::Option::None, align_y: ::std::option::Option::None, line_height: ::std::option::Option::None, shaping: ::std::option::Option::None, wrapping: ::std::option::Option::None, tracking: 0.0f32, font: ::std::option::Option::Some(::ui_lang_guest::wire::NamedFont { family: ::ui_lang_guest::wire::FontFamily::Named("Geist".into()), weight: ::ui_lang_guest::wire::Weight::Normal, stretch: ::ui_lang_guest::wire::FontStretch::Normal, style: ::ui_lang_guest::wire::FontStyle::Normal }) }, key: format!("{}/@text:786", __ice_use_scope), size: ::std::option::Option::Some(((12.5) as f32).max(f32::EPSILON).min(f32::MAX)), color: ::std::option::Option::Some({ let __color: ::iced::Color = __ice_palette.colors[5]; ::ui_lang_guest::wire::Rgba([__color.r, __color.g, __color.b, __color.a]) }), font: ::ui_lang_guest::wire::Font { monospace: false, weight: ::ui_lang_guest::wire::Weight::Normal }, width: ::std::option::Option::Some(::ui_lang_guest::wire::Length::Fill), align_x: ::std::option::Option::Some(::ui_lang_guest::wire::AlignX::Center), content: (crate::host::empty_scope_label(::std::convert::AsRef::as_ref(&(self.scope_target)))).to_string() };
#[cfg(test)]
drop(__ice_render_source);
__ice_rendered
}); } for (__ice_index, comment_group) in crate::host::scope_groups(self.comment_rows.clone(), ::std::convert::AsRef::as_ref(&(self.scope_target)), ::std::convert::AsRef::as_ref(&(self.active_page))).iter().enumerate() { let __for_scope = format!("{}/@for:1413({})", __ice_use_scope, __ice_index); __children.push({
#[cfg(test)]
let __ice_render_source_location = ::ui_lang_runtime::testing::Location::new("src/ui/pages.ice", 793, 1, "rendered view node");
#[cfg(test)]
let __ice_render_source = ::ui_lang_runtime::testing::push_render_source(__ice_render_source_location);
let __ice_rendered: __IceElement<'_, __PagesViewMessage> = { let mut __children: ::std::vec::Vec<__IceElement<'_, __PagesViewMessage>> = ::std::vec::Vec::new(); if ((self.scope_target).is_empty() && (comment_group.target != self.active_page)) { __children.push({
#[cfg(test)]
let __ice_render_source_location = ::ui_lang_runtime::testing::Location::new("src/ui/pages.ice", 806, 1, "rendered view node");
#[cfg(test)]
let __ice_render_source = ::ui_lang_runtime::testing::push_render_source(__ice_render_source_location);
let __ice_rendered: __IceElement<'_, __PagesViewMessage> = ::ui_lang_guest::wire::Node::Button { checked: ::std::option::Option::None, expanded: ::std::option::Option::None, description: ::std::option::Option::Some(::std::string::String::from(comment_group.anchor.to_owned())), key: format!("{}/@button:806", __for_scope), content: ::ui_lang_guest::wire::ButtonContent::Child(::std::boxed::Box::new({
#[cfg(test)]
let __ice_render_source_location = ::ui_lang_runtime::testing::Location::new("src/ui/pages.ice", 814, 1, "rendered view node");
#[cfg(test)]
let __ice_render_source = ::ui_lang_runtime::testing::push_render_source(__ice_render_source_location);
let __ice_rendered: __IceElement<'_, __PagesViewMessage> = ::ui_lang_guest::wire::Node::Text { options: ::ui_lang_guest::wire::TextOptions { height: ::std::option::Option::None, align_y: ::std::option::Option::None, line_height: ::std::option::Option::None, shaping: ::std::option::Option::None, wrapping: ::std::option::Option::Some(::ui_lang_guest::wire::Wrapping::None), tracking: 0.0f32, font: ::std::option::Option::Some(::ui_lang_guest::wire::NamedFont { family: ::ui_lang_guest::wire::FontFamily::Named("Geist Mono".into()), weight: ::ui_lang_guest::wire::Weight::Medium, stretch: ::ui_lang_guest::wire::FontStretch::Normal, style: ::ui_lang_guest::wire::FontStyle::Normal }) }, key: format!("{}/@text:814", __for_scope), size: ::std::option::Option::Some(((10.5) as f32).max(f32::EPSILON).min(f32::MAX)), color: ::std::option::Option::Some({ let __color: ::iced::Color = __ice_palette.colors[72]; ::ui_lang_guest::wire::Rgba([__color.r, __color.g, __color.b, __color.a]) }), font: ::ui_lang_guest::wire::Font { monospace: false, weight: ::ui_lang_guest::wire::Weight::Normal }, width: ::std::option::Option::Some(::ui_lang_guest::wire::Length::Fill), align_x: ::std::option::Option::None, content: (comment_group.anchor.to_owned()).to_string() };
#[cfg(test)]
drop(__ice_render_source);
__ice_rendered
})), label: ::std::option::Option::Some(::std::string::String::from("Comments on this block".to_owned())), on_press: if ((self.busy || (!(self.host_error).is_empty()))) { ::std::option::Option::None } else { ::std::option::Option::Some(::ui_lang_guest::slots::message((move |__event_0| __PagesViewMessage::NarrowCommentScope(__event_0))(comment_group.target.to_owned()))) }, width: ::std::option::Option::Some(::ui_lang_guest::wire::Length::Fill), height: ::std::option::Option::None, padding: ::std::option::Option::Some(::ui_lang_guest::wire::Edges::all((4.0) as f32)), style: ::ui_lang_guest::wire::ButtonStyle { preset: ::ui_lang_guest::wire::ButtonPreset::Primary, recipe: Some(::ui_lang_guest::wire::ButtonRecipe { base: ::ui_lang_guest::wire::Face { background: ::std::option::Option::Some({ let __color: ::iced::Color = ::iced::Color::from_rgba8(0, 0, 0, 0.000000); ::ui_lang_guest::wire::Rgba([__color.r, __color.g, __color.b, __color.a]) }), text: ::std::option::Option::Some({ let __color: ::iced::Color = __ice_palette.colors[4]; ::ui_lang_guest::wire::Rgba([__color.r, __color.g, __color.b, __color.a]) }), border: ::std::option::Option::Some(::ui_lang_guest::wire::Border { color: ::std::option::Option::None, width: ::std::option::Option::None, radius: ::std::option::Option::Some([8.0; 4]) }) }, hover_background: ::std::option::Option::Some({ let __color: ::iced::Color = __ice_palette.colors[14]; ::ui_lang_guest::wire::Rgba([__color.r, __color.g, __color.b, __color.a]) }), pressed_background: ::std::option::Option::Some({ let __color: ::iced::Color = __ice_palette.colors[39]; ::ui_lang_guest::wire::Rgba([__color.r, __color.g, __color.b, __color.a]) }), disabled_background: ::std::option::Option::None, disabled_text: ::std::option::Option::None, disabled_opacity: ::std::option::Option::Some(0.5f32), focus_ring: ::std::option::Option::Some({ let __color: ::iced::Color = __ice_palette.colors[42]; ::ui_lang_guest::wire::Rgba([__color.r, __color.g, __color.b, __color.a]) }), text_size: ::std::option::Option::Some(12.5f32), line_height: ::std::option::Option::None, font: Some(::ui_lang_guest::wire::NamedFont { family: ::ui_lang_guest::wire::FontFamily::Named("Geist".into()), weight: ::ui_lang_guest::wire::Weight::Semibold, stretch: ::ui_lang_guest::wire::FontStretch::Normal, style: ::ui_lang_guest::wire::FontStyle::Normal }) }), active: ::ui_lang_guest::wire::Face { background: ::std::option::Option::Some({ let __color: ::iced::Color = ::iced::Color::from_rgba8(0, 0, 0, 0.000000); ::ui_lang_guest::wire::Rgba([__color.r, __color.g, __color.b, __color.a]) }), text: ::std::option::Option::Some({ let __color: ::iced::Color = __ice_palette.colors[72]; ::ui_lang_guest::wire::Rgba([__color.r, __color.g, __color.b, __color.a]) }), border: ::std::option::Option::Some(::ui_lang_guest::wire::Border { color: ::std::option::Option::Some({ let __color: ::iced::Color = ::iced::Color::from_rgba8(0, 0, 0, 0.000000); ::ui_lang_guest::wire::Rgba([__color.r, __color.g, __color.b, __color.a]) }), width: ::std::option::Option::Some(((1.0) as f32).max(0.0).min(f32::MAX)), radius: ::std::option::Option::Some([((6.0) as f32).max(0.0).min(f32::MAX), ((6.0) as f32).max(0.0).min(f32::MAX), ((6.0) as f32).max(0.0).min(f32::MAX), ((6.0) as f32).max(0.0).min(f32::MAX)]) }) }, hovered: ::std::option::Option::Some(::ui_lang_guest::wire::Face { background: ::std::option::Option::Some({ let __color: ::iced::Color = { let mut __color = __ice_palette.colors[4]; __color.a = 0.060000; __color }; ::ui_lang_guest::wire::Rgba([__color.r, __color.g, __color.b, __color.a]) }), text: ::std::option::Option::Some({ let __color: ::iced::Color = __ice_palette.colors[4]; ::ui_lang_guest::wire::Rgba([__color.r, __color.g, __color.b, __color.a]) }), border: ::std::option::Option::None }), pressed: ::std::option::Option::Some(::ui_lang_guest::wire::Face { background: ::std::option::Option::Some({ let __color: ::iced::Color = { let mut __color = __ice_palette.colors[4]; __color.a = 0.100000; __color }; ::ui_lang_guest::wire::Rgba([__color.r, __color.g, __color.b, __color.a]) }), text: ::std::option::Option::Some({ let __color: ::iced::Color = __ice_palette.colors[4]; ::ui_lang_guest::wire::Rgba([__color.r, __color.g, __color.b, __color.a]) }), border: ::std::option::Option::None }), disabled: ::std::option::Option::None } };
#[cfg(test)]
drop(__ice_render_source);
__ice_rendered
}); } __children.push({
#[cfg(test)]
let __ice_render_source_location = ::ui_lang_runtime::testing::Location::new("src/ui/pages.ice", 824, 1, "rendered view node");
#[cfg(test)]
let __ice_render_source = ::ui_lang_runtime::testing::push_render_source(__ice_render_source_location);
let __ice_rendered: __IceElement<'_, __PagesViewMessage> = { let mut __children: ::std::vec::Vec<__IceElement<'_, __PagesViewMessage>> = ::std::vec::Vec::new(); for (__ice_index, group_thread) in comment_group.threads.iter().enumerate() { let __for_scope = format!("{}/@for:1446({})", __for_scope, __ice_index); __children.push({
#[cfg(test)]
let __ice_render_source_location = ::ui_lang_runtime::testing::Location::new("src/ui/pages.ice", 826, 1, "rendered view node");
#[cfg(test)]
let __ice_render_source = ::ui_lang_runtime::testing::push_render_source(__ice_render_source_location);
let __ice_rendered: __IceElement<'_, __PagesViewMessage> = ::ui_lang_runtime::grow_stack(|| self.__ice_component_use_12(__ice_palette, format!("{}/PageCommentThreadCard@1447", __for_scope), (move |__event_0| __PagesViewMessage::PostThreadReply(__event_0)).clone(), (move |__event_0, __event_1| __PagesViewMessage::ResolveThreadSubmit(__event_0, __event_1)).clone(), (move |__event_0| __PagesViewMessage::SelectReplyThread(__event_0)).clone(), (move |__event_0| __PagesViewMessage::ToggleThreadReplies(__event_0)).clone(), group_thread.clone(), (self.reply_thread == group_thread.id), crate::host::expanded(::std::convert::AsRef::as_ref(&(self.expanded_threads)), ::std::convert::AsRef::as_ref(&(group_thread.id)))));
#[cfg(test)]
drop(__ice_render_source);
__ice_rendered
}); } ::ui_lang_guest::wire::Node::Linear { max_width: ::std::option::Option::None, clip: false, key: format!("{}/@layout:824", __for_scope), wrap: None, axis: ::ui_lang_guest::wire::Axis::Column, spacing: ::std::option::Option::Some((16.0) as f32), padding: ::std::option::Option::None, width: ::std::option::Option::Some(::ui_lang_guest::wire::Length::Fill), height: ::std::option::Option::None, align: ::std::option::Option::None, background: ::std::option::Option::None, border: ::std::option::Option::None, children: __children } };
#[cfg(test)]
drop(__ice_render_source);
__ice_rendered
}); ::ui_lang_guest::wire::Node::Linear { max_width: ::std::option::Option::None, clip: false, key: format!("{}/@layout:793", __for_scope), wrap: None, axis: ::ui_lang_guest::wire::Axis::Column, spacing: ::std::option::Option::Some((8.0) as f32), padding: ::std::option::Option::None, width: ::std::option::Option::Some(::ui_lang_guest::wire::Length::Fill), height: ::std::option::Option::None, align: ::std::option::Option::None, background: ::std::option::Option::None, border: ::std::option::Option::None, children: __children } };
#[cfg(test)]
drop(__ice_render_source);
__ice_rendered
}); } if (!(crate::host::scope_resolved(self.comment_rows.clone(), ::std::convert::AsRef::as_ref(&(self.scope_target)))).is_empty()) { __children.push({
#[cfg(test)]
let __ice_render_source_location = ::ui_lang_runtime::testing::Location::new("src/ui/pages.ice", 833, 1, "rendered view node");
#[cfg(test)]
let __ice_render_source = ::ui_lang_runtime::testing::push_render_source(__ice_render_source_location);
let __ice_rendered: __IceElement<'_, __PagesViewMessage> = ::ui_lang_guest::wire::Node::Button { checked: ::std::option::Option::None, expanded: ::std::option::Option::Some(self.resolved_open), description: ::std::option::Option::None, key: format!("{}/@button:833", __ice_use_scope), content: ::ui_lang_guest::wire::ButtonContent::Child(::std::boxed::Box::new({
#[cfg(test)]
let __ice_render_source_location = ::ui_lang_runtime::testing::Location::new("src/ui/pages.ice", 841, 1, "rendered view node");
#[cfg(test)]
let __ice_render_source = ::ui_lang_runtime::testing::push_render_source(__ice_render_source_location);
let __ice_rendered: __IceElement<'_, __PagesViewMessage> = ::ui_lang_guest::wire::Node::Text { options: ::ui_lang_guest::wire::TextOptions { height: ::std::option::Option::None, align_y: ::std::option::Option::None, line_height: ::std::option::Option::None, shaping: ::std::option::Option::None, wrapping: ::std::option::Option::Some(::ui_lang_guest::wire::Wrapping::None), tracking: 0.0f32, font: ::std::option::Option::Some(::ui_lang_guest::wire::NamedFont { family: ::ui_lang_guest::wire::FontFamily::Named("Geist".into()), weight: ::ui_lang_guest::wire::Weight::Medium, stretch: ::ui_lang_guest::wire::FontStretch::Normal, style: ::ui_lang_guest::wire::FontStyle::Normal }) }, key: format!("{}/@text:841", __ice_use_scope), size: ::std::option::Option::Some(((11.0) as f32).max(f32::EPSILON).min(f32::MAX)), color: ::std::option::Option::Some({ let __color: ::iced::Color = __ice_palette.colors[5]; ::ui_lang_guest::wire::Rgba([__color.r, __color.g, __color.b, __color.a]) }), font: ::ui_lang_guest::wire::Font { monospace: false, weight: ::ui_lang_guest::wire::Weight::Normal }, width: ::std::option::Option::Some(::ui_lang_guest::wire::Length::Fill), align_x: ::std::option::Option::None, content: (crate::host::resolved_label(::std::convert::AsRef::as_ref(&(crate::host::scope_resolved(self.comment_rows.clone(), ::std::convert::AsRef::as_ref(&(self.scope_target))))))).to_string() };
#[cfg(test)]
drop(__ice_render_source);
__ice_rendered
})), label: ::std::option::Option::Some(::std::string::String::from("Resolved threads".to_owned())), on_press: if ((self.busy || (!(self.host_error).is_empty()))) { ::std::option::Option::None } else { ::std::option::Option::Some(::ui_lang_guest::slots::message((move || __PagesViewMessage::ToggleResolvedComments)())) }, width: ::std::option::Option::Some(::ui_lang_guest::wire::Length::Fill), height: ::std::option::Option::None, padding: ::std::option::Option::Some(::ui_lang_guest::wire::Edges::all((4.0) as f32)), style: ::ui_lang_guest::wire::ButtonStyle { preset: ::ui_lang_guest::wire::ButtonPreset::Primary, recipe: Some(::ui_lang_guest::wire::ButtonRecipe { base: ::ui_lang_guest::wire::Face { background: ::std::option::Option::Some({ let __color: ::iced::Color = ::iced::Color::from_rgba8(0, 0, 0, 0.000000); ::ui_lang_guest::wire::Rgba([__color.r, __color.g, __color.b, __color.a]) }), text: ::std::option::Option::Some({ let __color: ::iced::Color = __ice_palette.colors[4]; ::ui_lang_guest::wire::Rgba([__color.r, __color.g, __color.b, __color.a]) }), border: ::std::option::Option::Some(::ui_lang_guest::wire::Border { color: ::std::option::Option::None, width: ::std::option::Option::None, radius: ::std::option::Option::Some([8.0; 4]) }) }, hover_background: ::std::option::Option::Some({ let __color: ::iced::Color = __ice_palette.colors[14]; ::ui_lang_guest::wire::Rgba([__color.r, __color.g, __color.b, __color.a]) }), pressed_background: ::std::option::Option::Some({ let __color: ::iced::Color = __ice_palette.colors[39]; ::ui_lang_guest::wire::Rgba([__color.r, __color.g, __color.b, __color.a]) }), disabled_background: ::std::option::Option::None, disabled_text: ::std::option::Option::None, disabled_opacity: ::std::option::Option::Some(0.5f32), focus_ring: ::std::option::Option::Some({ let __color: ::iced::Color = __ice_palette.colors[42]; ::ui_lang_guest::wire::Rgba([__color.r, __color.g, __color.b, __color.a]) }), text_size: ::std::option::Option::Some(12.5f32), line_height: ::std::option::Option::None, font: Some(::ui_lang_guest::wire::NamedFont { family: ::ui_lang_guest::wire::FontFamily::Named("Geist".into()), weight: ::ui_lang_guest::wire::Weight::Semibold, stretch: ::ui_lang_guest::wire::FontStretch::Normal, style: ::ui_lang_guest::wire::FontStyle::Normal }) }), active: ::ui_lang_guest::wire::Face { background: ::std::option::Option::Some({ let __color: ::iced::Color = ::iced::Color::from_rgba8(0, 0, 0, 0.000000); ::ui_lang_guest::wire::Rgba([__color.r, __color.g, __color.b, __color.a]) }), text: ::std::option::Option::Some({ let __color: ::iced::Color = __ice_palette.colors[5]; ::ui_lang_guest::wire::Rgba([__color.r, __color.g, __color.b, __color.a]) }), border: ::std::option::Option::Some(::ui_lang_guest::wire::Border { color: ::std::option::Option::Some({ let __color: ::iced::Color = ::iced::Color::from_rgba8(0, 0, 0, 0.000000); ::ui_lang_guest::wire::Rgba([__color.r, __color.g, __color.b, __color.a]) }), width: ::std::option::Option::Some(((1.0) as f32).max(0.0).min(f32::MAX)), radius: ::std::option::Option::Some([((6.0) as f32).max(0.0).min(f32::MAX), ((6.0) as f32).max(0.0).min(f32::MAX), ((6.0) as f32).max(0.0).min(f32::MAX), ((6.0) as f32).max(0.0).min(f32::MAX)]) }) }, hovered: ::std::option::Option::Some(::ui_lang_guest::wire::Face { background: ::std::option::Option::Some({ let __color: ::iced::Color = { let mut __color = __ice_palette.colors[4]; __color.a = 0.060000; __color }; ::ui_lang_guest::wire::Rgba([__color.r, __color.g, __color.b, __color.a]) }), text: ::std::option::Option::Some({ let __color: ::iced::Color = __ice_palette.colors[4]; ::ui_lang_guest::wire::Rgba([__color.r, __color.g, __color.b, __color.a]) }), border: ::std::option::Option::None }), pressed: ::std::option::Option::Some(::ui_lang_guest::wire::Face { background: ::std::option::Option::Some({ let __color: ::iced::Color = { let mut __color = __ice_palette.colors[4]; __color.a = 0.100000; __color }; ::ui_lang_guest::wire::Rgba([__color.r, __color.g, __color.b, __color.a]) }), text: ::std::option::Option::Some({ let __color: ::iced::Color = __ice_palette.colors[4]; ::ui_lang_guest::wire::Rgba([__color.r, __color.g, __color.b, __color.a]) }), border: ::std::option::Option::None }), disabled: ::std::option::Option::None } };
#[cfg(test)]
drop(__ice_render_source);
__ice_rendered
}); } if self.resolved_open { __children.push({
#[cfg(test)]
let __ice_render_source_location = ::ui_lang_runtime::testing::Location::new("src/ui/pages.ice", 852, 1, "rendered view node");
#[cfg(test)]
let __ice_render_source = ::ui_lang_runtime::testing::push_render_source(__ice_render_source_location);
let __ice_rendered: __IceElement<'_, __PagesViewMessage> = { let mut __children: ::std::vec::Vec<__IceElement<'_, __PagesViewMessage>> = ::std::vec::Vec::new(); for (__ice_index, resolved_row) in crate::host::scope_resolved(self.comment_rows.clone(), ::std::convert::AsRef::as_ref(&(self.scope_target))).iter().enumerate() { let __for_scope = format!("{}/@for:1474({})", __ice_use_scope, __ice_index); __children.push({
#[cfg(test)]
let __ice_render_source_location = ::ui_lang_runtime::testing::Location::new("src/ui/pages.ice", 854, 1, "rendered view node");
#[cfg(test)]
let __ice_render_source = ::ui_lang_runtime::testing::push_render_source(__ice_render_source_location);
let __ice_rendered: __IceElement<'_, __PagesViewMessage> = ::ui_lang_runtime::grow_stack(|| self.__ice_component_use_13(__ice_palette, format!("{}/PageCommentThreadCard@1475", __for_scope), (move |__event_0| __PagesViewMessage::PostThreadReply(__event_0)).clone(), (move |__event_0, __event_1| __PagesViewMessage::ResolveThreadSubmit(__event_0, __event_1)).clone(), (move |__event_0| __PagesViewMessage::SelectReplyThread(__event_0)).clone(), (move |__event_0| __PagesViewMessage::ToggleThreadReplies(__event_0)).clone(), resolved_row.thread.clone(), crate::host::expanded(::std::convert::AsRef::as_ref(&(self.expanded_threads)), ::std::convert::AsRef::as_ref(&(resolved_row.thread.id)))));
#[cfg(test)]
drop(__ice_render_source);
__ice_rendered
}); } ::ui_lang_guest::wire::Node::Linear { max_width: ::std::option::Option::None, clip: false, key: format!("{}/@layout:852", __ice_use_scope), wrap: None, axis: ::ui_lang_guest::wire::Axis::Column, spacing: ::std::option::Option::Some((16.0) as f32), padding: ::std::option::Option::None, width: ::std::option::Option::Some(::ui_lang_guest::wire::Length::Fill), height: ::std::option::Option::None, align: ::std::option::Option::None, background: ::std::option::Option::None, border: ::std::option::Option::None, children: __children } };
#[cfg(test)]
drop(__ice_render_source);
__ice_rendered
}); } ::ui_lang_guest::wire::Node::Linear { max_width: ::std::option::Option::None, clip: false, key: format!("{}/@layout:784", __ice_use_scope), wrap: None, axis: ::ui_lang_guest::wire::Axis::Column, spacing: ::std::option::Option::Some((18.0) as f32), padding: ::std::option::Option::None, width: ::std::option::Option::Some(::ui_lang_guest::wire::Length::Fill), height: ::std::option::Option::None, align: ::std::option::Option::None, background: ::std::option::Option::None, border: ::std::option::Option::None, children: __children } };
#[cfg(test)]
drop(__ice_render_source);
__ice_rendered
}) };
#[cfg(test)]
drop(__ice_render_source);
__ice_rendered
}) };
#[cfg(test)]
drop(__ice_render_source);
__ice_rendered
}); ::ui_lang_guest::wire::Node::Linear { max_width: ::std::option::Option::None, clip: false, key: format!("{}/@layout:762", __ice_use_scope), wrap: None, axis: ::ui_lang_guest::wire::Axis::Column, spacing: ::std::option::Option::Some((10.0) as f32), padding: ::std::option::Option::Some(::ui_lang_guest::wire::Edges { top: (12.0) as f32, right: (12.0) as f32, bottom: (12.0) as f32, left: (12.0) as f32 }), width: ::std::option::Option::Some(::ui_lang_guest::wire::Length::Fill), height: ::std::option::Option::Some(::ui_lang_guest::wire::Length::Shrink), align: ::std::option::Option::None, background: ::std::option::Option::None, border: ::std::option::Option::None, children: __children } };
#[cfg(test)]
drop(__ice_render_source);
__ice_rendered
}); __children.push({
#[cfg(test)]
let __ice_render_source_location = ::ui_lang_runtime::testing::Location::new("src/ui/pages.ice", 866, 1, "rendered view node");
#[cfg(test)]
let __ice_render_source = ::ui_lang_runtime::testing::push_render_source(__ice_render_source_location);
let __ice_rendered: __IceElement<'_, __PagesViewMessage> = ::ui_lang_guest::wire::Node::Container { shadow: ::ui_lang_guest::wire::Shadow { color: ::std::option::Option::None, x: ::std::option::Option::None, y: ::std::option::Option::None, blur: ::std::option::Option::None }, max_width: ::std::option::Option::None, max_height: ::std::option::Option::None, clip: false, key: format!("{}/@container:866", __ice_use_scope), width: ::std::option::Option::Some(::ui_lang_guest::wire::Length::Fill), height: ::std::option::Option::Some(::ui_lang_guest::wire::Length::Fixed((1.0) as f32)), padding: ::std::option::Option::None, align_x: ::std::option::Option::None, align_y: ::std::option::Option::None, background: (::std::option::Option::Some({ let __color: ::iced::Color = __ice_palette.colors[60]; ::ui_lang_guest::wire::Rgba([__color.r, __color.g, __color.b, __color.a]) })).map(::ui_lang_guest::wire::Background::Color), border: ::std::option::Option::None, snap: ::std::option::Option::None, content: ::std::boxed::Box::new({
#[cfg(test)]
let __ice_render_source_location = ::ui_lang_runtime::testing::Location::new("src/ui/pages.ice", 871, 1, "rendered view node");
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
}); __children.push({
#[cfg(test)]
let __ice_render_source_location = ::ui_lang_runtime::testing::Location::new("src/ui/pages.ice", 872, 1, "rendered view node");
#[cfg(test)]
let __ice_render_source = ::ui_lang_runtime::testing::push_render_source(__ice_render_source_location);
let __ice_rendered: __IceElement<'_, __PagesViewMessage> = { let mut __children: ::std::vec::Vec<__IceElement<'_, __PagesViewMessage>> = ::std::vec::Vec::new(); __children.push({
#[cfg(test)]
let __ice_render_source_location = ::ui_lang_runtime::testing::Location::new("src/ui/pages.ice", 887, 1, "rendered view node");
#[cfg(test)]
let __ice_render_source = ::ui_lang_runtime::testing::push_render_source(__ice_render_source_location);
let __ice_rendered: __IceElement<'_, __PagesViewMessage> = ::ui_lang_guest::wire::Node::Container { shadow: ::ui_lang_guest::wire::Shadow { color: ::std::option::Option::None, x: ::std::option::Option::None, y: ::std::option::Option::None, blur: ::std::option::Option::None }, max_width: ::std::option::Option::None, max_height: ::std::option::Option::None, clip: true, key: format!("{}/@container:887", __ice_use_scope), width: ::std::option::Option::Some(::ui_lang_guest::wire::Length::Fill), height: ::std::option::Option::None, padding: ::std::option::Option::None, align_x: ::std::option::Option::None, align_y: ::std::option::Option::None, background: (::std::option::Option::None).map(::ui_lang_guest::wire::Background::Color), border: ::std::option::Option::None, snap: ::std::option::Option::None, content: ::std::boxed::Box::new({
#[cfg(test)]
let __ice_render_source_location = ::ui_lang_runtime::testing::Location::new("src/ui/pages.ice", 888, 1, "rendered view node");
#[cfg(test)]
let __ice_render_source = ::ui_lang_runtime::testing::push_render_source(__ice_render_source_location);
let __ice_rendered: __IceElement<'_, __PagesViewMessage> = ::ui_lang_guest::wire::Node::Text { options: ::ui_lang_guest::wire::TextOptions { height: ::std::option::Option::None, align_y: ::std::option::Option::None, line_height: ::std::option::Option::None, shaping: ::std::option::Option::None, wrapping: ::std::option::Option::Some(::ui_lang_guest::wire::Wrapping::Word), tracking: 0.0f32, font: ::std::option::Option::Some(::ui_lang_guest::wire::NamedFont { family: ::ui_lang_guest::wire::FontFamily::Named("Geist".into()), weight: ::ui_lang_guest::wire::Weight::Medium, stretch: ::ui_lang_guest::wire::FontStretch::Normal, style: ::ui_lang_guest::wire::FontStyle::Normal }) }, key: format!("{}/@text:888", __ice_use_scope), size: ::std::option::Option::Some(((11.0) as f32).max(f32::EPSILON).min(f32::MAX)), color: ::std::option::Option::Some({ let __color: ::iced::Color = __ice_palette.colors[5]; ::ui_lang_guest::wire::Rgba([__color.r, __color.g, __color.b, __color.a]) }), font: ::ui_lang_guest::wire::Font { monospace: false, weight: ::ui_lang_guest::wire::Weight::Normal }, width: ::std::option::Option::Some(::ui_lang_guest::wire::Length::Fill), align_x: ::std::option::Option::None, content: (crate::host::compose_hint_of(::std::convert::AsRef::as_ref(&(self.blocks)), ::std::convert::AsRef::as_ref(&(self.scope_target)), ::std::convert::AsRef::as_ref(&(self.active_page))).to_owned()).to_string() };
#[cfg(test)]
drop(__ice_render_source);
__ice_rendered
}) };
#[cfg(test)]
drop(__ice_render_source);
__ice_rendered
}); __children.push({
#[cfg(test)]
let __ice_render_source_location = ::ui_lang_runtime::testing::Location::new("src/ui/pages.ice", 895, 1, "rendered view node");
#[cfg(test)]
let __ice_render_source = ::ui_lang_runtime::testing::push_render_source(__ice_render_source_location);
let __ice_rendered: __IceElement<'_, __PagesViewMessage> = { let mut __children: ::std::vec::Vec<__IceElement<'_, __PagesViewMessage>> = ::std::vec::Vec::new(); __children.push({
#[cfg(test)]
let __ice_render_source_location = ::ui_lang_runtime::testing::Location::new("src/ui/pages.ice", 900, 1, "rendered view node");
#[cfg(test)]
let __ice_render_source = ::ui_lang_runtime::testing::push_render_source(__ice_render_source_location);
let __ice_rendered: __IceElement<'_, __PagesViewMessage> = { let __ice_node_scope = format!("{}/page-comment({})", __ice_node_scope, self.active_page); ::ui_lang_guest::wire::Node::Input { options: ::ui_lang_guest::wire::InputOptions { label: ("New page comment".to_owned()).to_string(), description: ::std::option::Option::None, disabled: ((self.busy || (!(self.host_error).is_empty())) || self.threads_loading), padding: ::std::option::Option::Some(::ui_lang_guest::wire::Edges::all((6.2) as f32)), text_size: ::std::option::Option::Some((13.0) as f32), line_height: ::std::option::Option::Some((1.2) as f32), align: ::std::option::Option::None, font: ::std::option::Option::Some(::ui_lang_guest::wire::NamedFont { family: ::ui_lang_guest::wire::FontFamily::Named("Geist".into()), weight: ::ui_lang_guest::wire::Weight::Normal, stretch: ::ui_lang_guest::wire::FontStretch::Normal, style: ::ui_lang_guest::wire::FontStyle::Normal }) }, key: __ice_node_scope.clone(), placeholder: ::std::string::String::from("Start a thread…".to_owned()), value: (self.block_comment_draft).to_string(), on_input: ::ui_lang_guest::slots::handler::<::std::string::String, __PagesViewMessage>(::std::boxed::Box::new({ let __route = __PagesViewMessage::__BindBlockCommentDraft as fn(::std::string::String) -> __PagesViewMessage; move |__sent: ::std::string::String| ::std::option::Option::Some(__route(__sent)) })), on_submit: ::std::option::Option::Some(::ui_lang_guest::slots::message((move || __PagesViewMessage::PostBlockCommentSubmit)())), width: ::std::option::Option::Some(::ui_lang_guest::wire::Length::Fill), secure: (false), style: ::std::boxed::Box::new(::ui_lang_guest::wire::InputStyle { utility: ::ui_lang_guest::wire::InputFace { background: ::std::option::Option::Some({ let __color: ::iced::Color = __ice_palette.colors[3]; ::ui_lang_guest::wire::Rgba([__color.r, __color.g, __color.b, __color.a]) }), border: Some(::ui_lang_guest::wire::Border { color: ::std::option::Option::Some({ let __color: ::iced::Color = __ice_palette.colors[39]; ::ui_lang_guest::wire::Rgba([__color.r, __color.g, __color.b, __color.a]) }), width: ::std::option::Option::Some(1f32), radius: ::std::option::Option::Some([10f32; 4]) }), ..Default::default() }, focus_border: ::std::option::Option::Some({ let __color: ::iced::Color = __ice_palette.colors[42]; ::ui_lang_guest::wire::Rgba([__color.r, __color.g, __color.b, __color.a]) }), focused_hovered: ::std::option::Option::None, active: ::ui_lang_guest::wire::InputFace { icon: ::std::option::Option::None, background: ::std::option::Option::Some({ let __color: ::iced::Color = ::iced::Color::from_rgba8(0, 0, 0, 0.000000); ::ui_lang_guest::wire::Rgba([__color.r, __color.g, __color.b, __color.a]) }), border: ::std::option::Option::Some(::ui_lang_guest::wire::Border { color: ::std::option::Option::Some({ let __color: ::iced::Color = { let mut __color = __ice_palette.colors[4]; __color.a = 0.080000; __color }; ::ui_lang_guest::wire::Rgba([__color.r, __color.g, __color.b, __color.a]) }), width: ::std::option::Option::Some(((1.0) as f32).max(0.0).min(f32::MAX)), radius: ::std::option::Option::Some([((7.0) as f32).max(0.0).min(f32::MAX), ((7.0) as f32).max(0.0).min(f32::MAX), ((7.0) as f32).max(0.0).min(f32::MAX), ((7.0) as f32).max(0.0).min(f32::MAX)]) }), value: ::std::option::Option::Some({ let __color: ::iced::Color = __ice_palette.colors[4]; ::ui_lang_guest::wire::Rgba([__color.r, __color.g, __color.b, __color.a]) }), placeholder: ::std::option::Option::Some({ let __color: ::iced::Color = __ice_palette.colors[5]; ::ui_lang_guest::wire::Rgba([__color.r, __color.g, __color.b, __color.a]) }), selection: ::std::option::Option::Some({ let __color: ::iced::Color = { let mut __color = __ice_palette.colors[4]; __color.a = 0.180000; __color }; ::ui_lang_guest::wire::Rgba([__color.r, __color.g, __color.b, __color.a]) }) }, hovered: ::std::option::Option::Some(::ui_lang_guest::wire::InputFace { icon: ::std::option::Option::None, background: ::std::option::Option::Some({ let __color: ::iced::Color = { let mut __color = __ice_palette.colors[4]; __color.a = 0.040000; __color }; ::ui_lang_guest::wire::Rgba([__color.r, __color.g, __color.b, __color.a]) }), border: ::std::option::Option::Some(::ui_lang_guest::wire::Border { color: ::std::option::Option::Some({ let __color: ::iced::Color = { let mut __color = __ice_palette.colors[4]; __color.a = 0.110000; __color }; ::ui_lang_guest::wire::Rgba([__color.r, __color.g, __color.b, __color.a]) }), width: ::std::option::Option::None, radius: ::std::option::Option::None }), value: ::std::option::Option::None, placeholder: ::std::option::Option::None, selection: ::std::option::Option::None }), focused: ::std::option::Option::Some(::ui_lang_guest::wire::InputFace { icon: ::std::option::Option::None, background: ::std::option::Option::Some({ let __color: ::iced::Color = { let mut __color = __ice_palette.colors[4]; __color.a = 0.040000; __color }; ::ui_lang_guest::wire::Rgba([__color.r, __color.g, __color.b, __color.a]) }), border: ::std::option::Option::Some(::ui_lang_guest::wire::Border { color: ::std::option::Option::Some({ let __color: ::iced::Color = __ice_palette.colors[42]; ::ui_lang_guest::wire::Rgba([__color.r, __color.g, __color.b, __color.a]) }), width: ::std::option::Option::None, radius: ::std::option::Option::None }), value: ::std::option::Option::None, placeholder: ::std::option::Option::None, selection: ::std::option::Option::None }), disabled: ::std::option::Option::Some(::ui_lang_guest::wire::InputFace { icon: ::std::option::Option::None, background: ::std::option::Option::None, border: ::std::option::Option::None, value: ::std::option::Option::Some({ let __color: ::iced::Color = __ice_palette.colors[5]; ::ui_lang_guest::wire::Rgba([__color.r, __color.g, __color.b, __color.a]) }), placeholder: ::std::option::Option::None, selection: ::std::option::Option::None }) }) } };
#[cfg(test)]
drop(__ice_render_source);
__ice_rendered
}); __children.push({
#[cfg(test)]
let __ice_render_source_location = ::ui_lang_runtime::testing::Location::new("src/ui/pages.ice", 915, 1, "rendered view node");
#[cfg(test)]
let __ice_render_source = ::ui_lang_runtime::testing::push_render_source(__ice_render_source_location);
let __ice_rendered: __IceElement<'_, __PagesViewMessage> = { let __ice_node_scope = format!("{}/post", __ice_node_scope); ::ui_lang_guest::wire::Node::Button { checked: ::std::option::Option::None, expanded: ::std::option::Option::None, description: ::std::option::Option::None, key: __ice_node_scope.clone(), content: ::ui_lang_guest::wire::ButtonContent::Label(::std::string::String::from("Post")), label: ::std::option::Option::None, on_press: if ((((self.busy || (!(self.host_error).is_empty())) || ((self.block_comment_draft).trim().to_owned()).is_empty()) || self.threads_loading)) { ::std::option::Option::None } else { ::std::option::Option::Some(::ui_lang_guest::slots::message((move || __PagesViewMessage::PostBlockCommentSubmit)())) }, width: ::std::option::Option::None, height: ::std::option::Option::None, padding: ::std::option::Option::Some(::ui_lang_guest::wire::Edges::all((5.0) as f32)), style: ::ui_lang_guest::wire::ButtonStyle { preset: ::ui_lang_guest::wire::ButtonPreset::Primary, recipe: Some(::ui_lang_guest::wire::ButtonRecipe { base: ::ui_lang_guest::wire::Face { background: ::std::option::Option::Some({ let __color: ::iced::Color = __ice_palette.colors[7]; ::ui_lang_guest::wire::Rgba([__color.r, __color.g, __color.b, __color.a]) }), text: ::std::option::Option::Some({ let __color: ::iced::Color = __ice_palette.colors[9]; ::ui_lang_guest::wire::Rgba([__color.r, __color.g, __color.b, __color.a]) }), border: ::std::option::Option::Some(::ui_lang_guest::wire::Border { color: ::std::option::Option::None, width: ::std::option::Option::None, radius: ::std::option::Option::Some([9.0; 4]) }) }, hover_background: ::std::option::Option::Some({ let __color: ::iced::Color = __ice_palette.colors[8]; ::ui_lang_guest::wire::Rgba([__color.r, __color.g, __color.b, __color.a]) }), pressed_background: ::std::option::Option::Some({ let __color: ::iced::Color = { let mut __color = __ice_palette.colors[7]; __color.a = 0.800000; __color }; ::ui_lang_guest::wire::Rgba([__color.r, __color.g, __color.b, __color.a]) }), disabled_background: ::std::option::Option::Some({ let __color: ::iced::Color = __ice_palette.colors[10]; ::ui_lang_guest::wire::Rgba([__color.r, __color.g, __color.b, __color.a]) }), disabled_text: ::std::option::Option::Some({ let __color: ::iced::Color = __ice_palette.colors[11]; ::ui_lang_guest::wire::Rgba([__color.r, __color.g, __color.b, __color.a]) }), disabled_opacity: ::std::option::Option::None, focus_ring: ::std::option::Option::Some({ let __color: ::iced::Color = __ice_palette.colors[42]; ::ui_lang_guest::wire::Rgba([__color.r, __color.g, __color.b, __color.a]) }), text_size: ::std::option::Option::Some(12.5f32), line_height: ::std::option::Option::None, font: Some(::ui_lang_guest::wire::NamedFont { family: ::ui_lang_guest::wire::FontFamily::Named("Geist".into()), weight: ::ui_lang_guest::wire::Weight::Semibold, stretch: ::ui_lang_guest::wire::FontStretch::Normal, style: ::ui_lang_guest::wire::FontStyle::Normal }) }), active: ::ui_lang_guest::wire::Face::default(), hovered: ::std::option::Option::None, pressed: ::std::option::Option::None, disabled: ::std::option::Option::None } } };
#[cfg(test)]
drop(__ice_render_source);
__ice_rendered
}); ::ui_lang_guest::wire::Node::Linear { max_width: ::std::option::Option::None, clip: false, key: format!("{}/@layout:895", __ice_use_scope), wrap: None, axis: ::ui_lang_guest::wire::Axis::Row, spacing: ::std::option::Option::Some((5.0) as f32), padding: ::std::option::Option::None, width: ::std::option::Option::Some(::ui_lang_guest::wire::Length::Fill), height: ::std::option::Option::None, align: ::std::option::Option::Some(::ui_lang_guest::wire::AlignX::Center), background: ::std::option::Option::None, border: ::std::option::Option::None, children: __children } };
#[cfg(test)]
drop(__ice_render_source);
__ice_rendered
}); ::ui_lang_guest::wire::Node::Linear { max_width: ::std::option::Option::None, clip: false, key: format!("{}/@layout:872", __ice_use_scope), wrap: None, axis: ::ui_lang_guest::wire::Axis::Column, spacing: ::std::option::Option::Some((6.0) as f32), padding: ::std::option::Option::Some(::ui_lang_guest::wire::Edges { top: (12.0) as f32, right: (12.0) as f32, bottom: (12.0) as f32, left: (12.0) as f32 }), width: ::std::option::Option::Some(::ui_lang_guest::wire::Length::Fill), height: ::std::option::Option::Some(::ui_lang_guest::wire::Length::Shrink), align: ::std::option::Option::None, background: ::std::option::Option::None, border: ::std::option::Option::None, children: __children } };
#[cfg(test)]
drop(__ice_render_source);
__ice_rendered
}); ::ui_lang_guest::wire::Node::Linear { max_width: ::std::option::Option::None, clip: false, key: format!("{}/@layout:694", __ice_use_scope), wrap: None, axis: ::ui_lang_guest::wire::Axis::Column, spacing: ::std::option::Option::None, padding: ::std::option::Option::None, width: ::std::option::Option::Some(::ui_lang_guest::wire::Length::Fill), height: ::std::option::Option::Some(::ui_lang_guest::wire::Length::Shrink), align: ::std::option::Option::None, background: ::std::option::Option::None, border: ::std::option::Option::None, children: __children } };
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
}) };
#[cfg(test)]
drop(__ice_render_source);
__ice_rendered
}); } __children.push({
#[cfg(test)]
let __ice_render_source_location = ::ui_lang_runtime::testing::Location::new("src/ui/pages.ice", 926, 1, "rendered view node");
#[cfg(test)]
let __ice_render_source = ::ui_lang_runtime::testing::push_render_source(__ice_render_source_location);
let __ice_rendered: __IceElement<'_, __PagesViewMessage> = { let mut __children = vec![{
#[cfg(test)]
let __ice_render_source_location = ::ui_lang_runtime::testing::Location::new("src/ui/pages.ice", 935, 1, "rendered view node");
#[cfg(test)]
let __ice_render_source = ::ui_lang_runtime::testing::push_render_source(__ice_render_source_location);
let __ice_rendered: __IceElement<'_, __PagesViewMessage> = ::ui_lang_guest::wire::Node::Space { width: ::std::option::Option::Some(::ui_lang_guest::wire::Length::Fill), height: ::std::option::Option::Some(::ui_lang_guest::wire::Length::Fill) };
#[cfg(test)]
drop(__ice_render_source);
__ice_rendered
}]; if self.page_menu_open { __children.push({
#[cfg(test)]
let __ice_render_source_location = ::ui_lang_runtime::testing::Location::new("src/ui/pages.ice", 937, 1, "rendered view node");
#[cfg(test)]
let __ice_render_source = ::ui_lang_runtime::testing::push_render_source(__ice_render_source_location);
let __ice_rendered: __IceElement<'_, __PagesViewMessage> = { let __ice_node_scope = format!("{}/page-menu", __ice_use_scope); ::ui_lang_guest::wire::Node::Container { shadow: ::ui_lang_guest::wire::Shadow { color: ::std::option::Option::Some({ let __color: ::iced::Color = __ice_palette.colors[46]; ::ui_lang_guest::wire::Rgba([__color.r, __color.g, __color.b, __color.a]) }), x: ::std::option::Option::None, y: ::std::option::Option::Some((8.0) as f32), blur: ::std::option::Option::Some((24.0) as f32) }, max_width: ::std::option::Option::None, max_height: ::std::option::Option::None, clip: false, key: __ice_node_scope.clone(), width: ::std::option::Option::Some(::ui_lang_guest::wire::Length::Fixed((186.0) as f32)), height: ::std::option::Option::None, padding: ::std::option::Option::Some(::ui_lang_guest::wire::Edges { top: (4.0) as f32, right: (4.0) as f32, bottom: (4.0) as f32, left: (4.0) as f32 }), align_x: ::std::option::Option::None, align_y: ::std::option::Option::None, background: (::std::option::Option::Some({ let __color: ::iced::Color = __ice_palette.colors[55]; ::ui_lang_guest::wire::Rgba([__color.r, __color.g, __color.b, __color.a]) })).map(::ui_lang_guest::wire::Background::Color), border: ::std::option::Option::Some(::ui_lang_guest::wire::Border { color: ::std::option::Option::Some({ let __color: ::iced::Color = { let mut __color = __ice_palette.colors[4]; __color.a = 0.100000; __color }; ::ui_lang_guest::wire::Rgba([__color.r, __color.g, __color.b, __color.a]) }), width: ::std::option::Option::Some(((1.0) as f32).max(0.0).min(f32::MAX)), radius: ::std::option::Option::Some([((10.0) as f32).max(0.0).min(f32::MAX), ((10.0) as f32).max(0.0).min(f32::MAX), ((10.0) as f32).max(0.0).min(f32::MAX), ((10.0) as f32).max(0.0).min(f32::MAX)]) }), snap: ::std::option::Option::None, content: ::std::boxed::Box::new({
#[cfg(test)]
let __ice_render_source_location = ::ui_lang_runtime::testing::Location::new("src/ui/pages.ice", 951, 1, "rendered view node");
#[cfg(test)]
let __ice_render_source = ::ui_lang_runtime::testing::push_render_source(__ice_render_source_location);
let __ice_rendered: __IceElement<'_, __PagesViewMessage> = ::ui_lang_guest::wire::Node::Button { checked: ::std::option::Option::None, expanded: ::std::option::Option::None, description: ::std::option::Option::None, key: format!("{}/@button:951", __ice_use_scope), content: ::ui_lang_guest::wire::ButtonContent::Label(::std::string::String::from("Delete page…")), label: ::std::option::Option::Some(::std::string::String::from("Delete page".to_owned())), on_press: if ((self.busy || (!(self.host_error).is_empty()))) { ::std::option::Option::None } else { ::std::option::Option::Some(::ui_lang_guest::slots::message((move || __PagesViewMessage::ArmPageDelete)())) }, width: ::std::option::Option::Some(::ui_lang_guest::wire::Length::Fill), height: ::std::option::Option::None, padding: ::std::option::Option::Some(::ui_lang_guest::wire::Edges::all((6.0) as f32)), style: ::ui_lang_guest::wire::ButtonStyle { preset: ::ui_lang_guest::wire::ButtonPreset::Primary, recipe: Some(::ui_lang_guest::wire::ButtonRecipe { base: ::ui_lang_guest::wire::Face { background: ::std::option::Option::Some({ let __color: ::iced::Color = __ice_palette.colors[20]; ::ui_lang_guest::wire::Rgba([__color.r, __color.g, __color.b, __color.a]) }), text: ::std::option::Option::Some({ let __color: ::iced::Color = __ice_palette.colors[21]; ::ui_lang_guest::wire::Rgba([__color.r, __color.g, __color.b, __color.a]) }), border: ::std::option::Option::Some(::ui_lang_guest::wire::Border { color: ::std::option::Option::None, width: ::std::option::Option::None, radius: ::std::option::Option::Some([9.0; 4]) }) }, hover_background: ::std::option::Option::Some({ let __color: ::iced::Color = { let mut __color = __ice_palette.colors[20]; __color.a = 0.900000; __color }; ::ui_lang_guest::wire::Rgba([__color.r, __color.g, __color.b, __color.a]) }), pressed_background: ::std::option::Option::Some({ let __color: ::iced::Color = { let mut __color = __ice_palette.colors[20]; __color.a = 0.800000; __color }; ::ui_lang_guest::wire::Rgba([__color.r, __color.g, __color.b, __color.a]) }), disabled_background: ::std::option::Option::None, disabled_text: ::std::option::Option::None, disabled_opacity: ::std::option::Option::Some(0.5f32), focus_ring: ::std::option::Option::Some({ let __color: ::iced::Color = __ice_palette.colors[42]; ::ui_lang_guest::wire::Rgba([__color.r, __color.g, __color.b, __color.a]) }), text_size: ::std::option::Option::Some(12.5f32), line_height: ::std::option::Option::None, font: Some(::ui_lang_guest::wire::NamedFont { family: ::ui_lang_guest::wire::FontFamily::Named("Geist".into()), weight: ::ui_lang_guest::wire::Weight::Semibold, stretch: ::ui_lang_guest::wire::FontStretch::Normal, style: ::ui_lang_guest::wire::FontStyle::Normal }) }), active: ::ui_lang_guest::wire::Face { background: ::std::option::Option::Some({ let __color: ::iced::Color = ::iced::Color::from_rgba8(0, 0, 0, 0.000000); ::ui_lang_guest::wire::Rgba([__color.r, __color.g, __color.b, __color.a]) }), text: ::std::option::Option::Some({ let __color: ::iced::Color = __ice_palette.colors[20]; ::ui_lang_guest::wire::Rgba([__color.r, __color.g, __color.b, __color.a]) }), border: ::std::option::Option::Some(::ui_lang_guest::wire::Border { color: ::std::option::Option::Some({ let __color: ::iced::Color = ::iced::Color::from_rgba8(0, 0, 0, 0.000000); ::ui_lang_guest::wire::Rgba([__color.r, __color.g, __color.b, __color.a]) }), width: ::std::option::Option::Some(((1.0) as f32).max(0.0).min(f32::MAX)), radius: ::std::option::Option::Some([((7.0) as f32).max(0.0).min(f32::MAX), ((7.0) as f32).max(0.0).min(f32::MAX), ((7.0) as f32).max(0.0).min(f32::MAX), ((7.0) as f32).max(0.0).min(f32::MAX)]) }) }, hovered: ::std::option::Option::Some(::ui_lang_guest::wire::Face { background: ::std::option::Option::Some({ let __color: ::iced::Color = __ice_palette.colors[22]; ::ui_lang_guest::wire::Rgba([__color.r, __color.g, __color.b, __color.a]) }), text: ::std::option::Option::Some({ let __color: ::iced::Color = __ice_palette.colors[20]; ::ui_lang_guest::wire::Rgba([__color.r, __color.g, __color.b, __color.a]) }), border: ::std::option::Option::Some(::ui_lang_guest::wire::Border { color: ::std::option::Option::Some({ let __color: ::iced::Color = __ice_palette.colors[23]; ::ui_lang_guest::wire::Rgba([__color.r, __color.g, __color.b, __color.a]) }), width: ::std::option::Option::None, radius: ::std::option::Option::None }) }), pressed: ::std::option::Option::Some(::ui_lang_guest::wire::Face { background: ::std::option::Option::Some({ let __color: ::iced::Color = __ice_palette.colors[23]; ::ui_lang_guest::wire::Rgba([__color.r, __color.g, __color.b, __color.a]) }), text: ::std::option::Option::Some({ let __color: ::iced::Color = __ice_palette.colors[4]; ::ui_lang_guest::wire::Rgba([__color.r, __color.g, __color.b, __color.a]) }), border: ::std::option::Option::None }), disabled: ::std::option::Option::None } };
#[cfg(test)]
drop(__ice_render_source);
__ice_rendered
}) } };
#[cfg(test)]
drop(__ice_render_source);
__ice_rendered
}); } ::ui_lang_guest::wire::Node::Overlay { key: format!("{}/@overlay:926", __ice_use_scope), padding: (8.0) as f32, backdrop: { let __color: ::iced::Color = ::iced::Color::from_rgba8(0, 0, 0, 0.000000); ::ui_lang_guest::wire::Rgba([__color.r, __color.g, __color.b, __color.a]) }, align_x: ::ui_lang_guest::wire::AlignX::Right, align_y: ::ui_lang_guest::wire::AlignY::Top, on_dismiss: ::std::option::Option::Some(::ui_lang_guest::slots::message((move || __PagesViewMessage::ClosePageMenu)())), children: __children } };
#[cfg(test)]
drop(__ice_render_source);
__ice_rendered
}); __children.push({
#[cfg(test)]
let __ice_render_source_location = ::ui_lang_runtime::testing::Location::new("src/ui/pages.ice", 961, 1, "rendered view node");
#[cfg(test)]
let __ice_render_source = ::ui_lang_runtime::testing::push_render_source(__ice_render_source_location);
let __ice_rendered: __IceElement<'_, __PagesViewMessage> = { let mut __children = vec![{
#[cfg(test)]
let __ice_render_source_location = ::ui_lang_runtime::testing::Location::new("src/ui/pages.ice", 970, 1, "rendered view node");
#[cfg(test)]
let __ice_render_source = ::ui_lang_runtime::testing::push_render_source(__ice_render_source_location);
let __ice_rendered: __IceElement<'_, __PagesViewMessage> = ::ui_lang_guest::wire::Node::Space { width: ::std::option::Option::Some(::ui_lang_guest::wire::Length::Fill), height: ::std::option::Option::Some(::ui_lang_guest::wire::Length::Fill) };
#[cfg(test)]
drop(__ice_render_source);
__ice_rendered
}]; if self.page_delete_armed { __children.push({
#[cfg(test)]
let __ice_render_source_location = ::ui_lang_runtime::testing::Location::new("src/ui/pages.ice", 979, 1, "rendered view node");
#[cfg(test)]
let __ice_render_source = ::ui_lang_runtime::testing::push_render_source(__ice_render_source_location);
let __ice_rendered: __IceElement<'_, __PagesViewMessage> = ::ui_lang_runtime::grow_stack(|| self.__ice_component_use_15(__ice_palette, format!("{}/ConfirmDelete@1600", __ice_use_scope), (move || __PagesViewMessage::ArmPageDelete).clone(), (move |__event_0| __PagesViewMessage::ChoosePage(__event_0)).clone(), (move || __PagesViewMessage::ClearPageSearch).clone(), (move || __PagesViewMessage::CloseBlockComments).clone(), (move || __PagesViewMessage::ClosePageMenu).clone(), (move |__event_0, __event_1| __PagesViewMessage::CopyToClipboard(__event_0, __event_1)).clone(), (move || __PagesViewMessage::CreatePageSubmit).clone(), (move || __PagesViewMessage::DeletePageSubmit).clone(), (move || __PagesViewMessage::DisarmPageDelete).clone(), (move |__event_0| __PagesViewMessage::DiscardOrphanedCommentDraft(__event_0)).clone(), (move |__event_0, __event_1| __PagesViewMessage::CommentsCardMeasured(__event_0, __event_1)).clone(), (move |__event_0| __PagesViewMessage::NarrowCommentScope(__event_0)).clone(), (move |__event_0, __event_1| __PagesViewMessage::OpenPageSearchHit(__event_0, __event_1)).clone(), (move || __PagesViewMessage::PostBlockCommentSubmit).clone(), (move |__event_0| __PagesViewMessage::PostThreadReply(__event_0)).clone(), (move |__event_0, __event_1| __PagesViewMessage::PagesPaneResized(__event_0, __event_1)).clone(), (move |__event_0, __event_1| __PagesViewMessage::SidebarResized(__event_0, __event_1)).clone(), (move |__event_0, __event_1| __PagesViewMessage::ResolveThreadSubmit(__event_0, __event_1)).clone(), (move || __PagesViewMessage::SearchPagesSubmit).clone(), (move |__event_0| __PagesViewMessage::SelectReplyThread(__event_0)).clone(), (move || __PagesViewMessage::ToggleBlockComments).clone(), (move || __PagesViewMessage::TogglePageCreate).clone(), (move || __PagesViewMessage::TogglePageMenu).clone(), (move || __PagesViewMessage::ToggleResolvedComments).clone(), (move |__event_0| __PagesViewMessage::ToggleThreadReplies(__event_0)).clone(), (move |__event_0| __PagesViewMessage::UseOrphanedCommentDraft(__event_0)).clone(), (move || __PagesViewMessage::WidenCommentScope).clone()));
#[cfg(test)]
drop(__ice_render_source);
__ice_rendered
}); } ::ui_lang_guest::wire::Node::Overlay { key: format!("{}/@overlay:961", __ice_use_scope), padding: (30.0) as f32, backdrop: { let __color: ::iced::Color = __ice_palette.colors[85]; ::ui_lang_guest::wire::Rgba([__color.r, __color.g, __color.b, __color.a]) }, align_x: ::ui_lang_guest::wire::AlignX::Center, align_y: ::ui_lang_guest::wire::AlignY::Center, on_dismiss: ::std::option::Option::Some(::ui_lang_guest::slots::message((move || __PagesViewMessage::DisarmPageDelete)())), children: __children } };
#[cfg(test)]
drop(__ice_render_source);
__ice_rendered
}); ::ui_lang_guest::wire::Node::Stack { key: format!("{}/@layout:387", __ice_use_scope), width: ::std::option::Option::Some(::ui_lang_guest::wire::Length::Fill), height: ::std::option::Option::Some(::ui_lang_guest::wire::Length::Fill), padding: ::std::option::Option::None, background: ::std::option::Option::None, border: ::std::option::Option::None, clip: true, under: 0u32, children: __children } };
#[cfg(test)]
drop(__ice_render_source);
__ice_rendered
}); ::ui_lang_guest::wire::Node::Linear { max_width: ::std::option::Option::None, clip: false, key: format!("{}/@layout:154", __ice_use_scope), wrap: None, axis: ::ui_lang_guest::wire::Axis::Column, spacing: ::std::option::Option::None, padding: ::std::option::Option::None, width: ::std::option::Option::Some(::ui_lang_guest::wire::Length::Fill), height: ::std::option::Option::Some(::ui_lang_guest::wire::Length::Fill), align: ::std::option::Option::None, background: ::std::option::Option::None, border: ::std::option::Option::None, children: __children } };
#[cfg(test)]
drop(__ice_render_source);
__ice_rendered
}); ::ui_lang_guest::wire::Node::Linear { max_width: ::std::option::Option::None, clip: false, key: format!("{}/@layout:153", __ice_use_scope), wrap: None, axis: ::ui_lang_guest::wire::Axis::Row, spacing: ::std::option::Option::None, padding: ::std::option::Option::None, width: ::std::option::Option::Some(::ui_lang_guest::wire::Length::Fill), height: ::std::option::Option::Some(::ui_lang_guest::wire::Length::Fill), align: ::std::option::Option::None, background: ::std::option::Option::None, border: ::std::option::Option::None, children: __children } };
#[cfg(test)]
drop(__ice_render_source);
__ice_rendered
}); ::ui_lang_guest::wire::Node::Linear { max_width: ::std::option::Option::None, clip: false, key: format!("{}/@layout:33", __ice_use_scope), wrap: None, axis: ::ui_lang_guest::wire::Axis::Row, spacing: ::std::option::Option::None, padding: ::std::option::Option::None, width: ::std::option::Option::Some(::ui_lang_guest::wire::Length::Fill), height: ::std::option::Option::Some(::ui_lang_guest::wire::Length::Fill), align: ::std::option::Option::None, background: ::std::option::Option::None, border: ::std::option::Option::None, children: __children } };
#[cfg(test)]
drop(__ice_render_source);
__ice_rendered
}; __component_content }
}
}
