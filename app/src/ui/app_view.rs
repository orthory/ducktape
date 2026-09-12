#[allow(warnings, clippy::all)]
mod __ice_group_app_view {
use super::*;
impl super::Ducktape {
pub(super) fn __view(&self, window: ::iced::window::Id) -> __IceElement<'_, __DucktapeMessage> { let __ice_view = ::ui_lang_runtime::dev::Span::view("Ducktape", "view.ice:7"); let __ice_palette = self.__palette(window); let __ice_app_theme = Self::__app_theme(__ice_palette); let __ice_content: __IceElement<'_, __DucktapeMessage> = { static __ICE_TEMPLATE_JSON: &str = "{\n  \"root\": {\n    \"kind\": \"linear\",\n    \"a11y\": {\n      \"segment\": \"@layout:7\",\n      \"named\": false,\n      \"source\": {\n        \"path\": 0,\n        \"line\": 7,\n        \"column\": 1\n      }\n    },\n    \"axis\": \"column\",\n    \"width\": \"fill\",\n    \"height\": \"fill\",\n    \"children\": [\n      {\n        \"kind\": \"group\",\n        \"slot\": 0\n      },\n      {\n        \"kind\": \"group\",\n        \"slot\": 1\n      },\n      {\n        \"kind\": \"group\",\n        \"slot\": 2\n      }\n    ]\n  },\n  \"slots\": {\n    \"texts\": 0,\n    \"states\": 0,\n    \"messages\": 0,\n    \"handlers\": 0,\n    \"subtrees\": 0,\n    \"groups\": 3,\n    \"bools\": 0\n  }\n}"; static __ICE_TEMPLATE_PATHS: [&str; 1] = ["view.ice",]; thread_local! { static __ICE_TEMPLATE: ::ui_lang_runtime::template::TemplateSource = ::ui_lang_runtime::template::TemplateSource::new(__ICE_TEMPLATE_JSON); } let __ice_template = __ICE_TEMPLATE.with(|source| source.current()); let mut __ice_slots: ::ui_lang_runtime::template::Slots<'_, __DucktapeMessage> = ::ui_lang_runtime::template::Slots::with_capacity(::ui_lang_runtime::template::SlotCounts { texts: 0, states: 0, messages: 0, handlers: 0, subtrees: 0, groups: 3, bools: 0 }); self.__ice_slots_0(__ice_palette, &__ice_app_theme, window, &mut __ice_slots);
 ::ui_lang_runtime::template::render(&__ice_template, &__ice_slots, &__ice_palette.colors, "Ducktape", &__ICE_TEMPLATE_PATHS) }; let __ice_root: __IceElement<'_, __DucktapeMessage> = ::ui_lang_runtime::navigation(__ice_content, __DucktapeMessage::__AccessibilityFocusNext(::std::option::Option::Some(window)), __DucktapeMessage::__AccessibilityFocusPrevious(::std::option::Option::Some(window))).in_window(window).into(); ::ui_lang_runtime::dev::ready(__ice_root) }
pub(super) fn __ice_slots_0<'a>(&'a self, __ice_palette: __IcePalette, __ice_app_theme: &::iced::Theme, window: ::iced::window::Id, __ice_slots: &mut ::ui_lang_runtime::template::Slots<'a, __DucktapeMessage>) { let __ice_app_theme = __ice_app_theme.clone(); let _ = &__ice_app_theme; __ice_slots.push_group({ let mut __children: ::std::vec::Vec<__IceElement<'_, __DucktapeMessage>> = ::std::vec::Vec::new(); if ((self.console_win != ::std::option::Option::Some(window)) && (self.huddle_win != ::std::option::Option::Some(window))) { __children.push({
#[cfg(test)]
let __ice_render_source_location = ::ui_lang_runtime::testing::Location::new("view.ice", 9, 1, "rendered view node");
#[cfg(test)]
let __ice_render_source = ::ui_lang_runtime::testing::push_render_source(__ice_render_source_location);
let __ice_rendered: __IceElement<'_, __DucktapeMessage> = { let __ice_component_0487562436f6c756d6e_scope_10219 = format!("{}/HubColumn@10219", "Ducktape"); let __component_content: __IceElement<'_, __DucktapeMessage> = {
#[cfg(test)]
let __ice_render_source_location = ::ui_lang_runtime::testing::Location::new("components/onboarding.ice", 47, 1, "rendered view node");
#[cfg(test)]
let __ice_render_source = ::ui_lang_runtime::testing::push_render_source(__ice_render_source_location);
let __ice_rendered: __IceElement<'_, __DucktapeMessage> = { let __ice_node_scope = format!("{}/root", __ice_component_0487562436f6c756d6e_scope_10219); { let __a11y_key = __ice_node_scope.as_str(); let __container_content: __IceElement<'_, __DucktapeMessage> = {
#[cfg(test)]
let __ice_render_source_location = ::ui_lang_runtime::testing::Location::new("components/onboarding.ice", 52, 1, "rendered view node");
#[cfg(test)]
let __ice_render_source = ::ui_lang_runtime::testing::push_render_source(__ice_render_source_location);
let __ice_rendered: __IceElement<'_, __DucktapeMessage> = { let mut __children: ::std::vec::Vec<__IceElement<'_, __DucktapeMessage>> = ::std::vec::Vec::new(); __children.push({
#[cfg(test)]
let __ice_render_source_location = ::ui_lang_runtime::testing::Location::new("components/onboarding.ice", 58, 1, "rendered view node");
#[cfg(test)]
let __ice_render_source = ::ui_lang_runtime::testing::push_render_source(__ice_render_source_location);
let __ice_rendered: __IceElement<'_, __DucktapeMessage> = { let mut __children: ::std::vec::Vec<__IceElement<'_, __DucktapeMessage>> = ::std::vec::Vec::new(); __children.push({
#[cfg(test)]
let __ice_render_source_location = ::ui_lang_runtime::testing::Location::new("components/onboarding.ice", 64, 1, "rendered view node");
#[cfg(test)]
let __ice_render_source = ::ui_lang_runtime::testing::push_render_source(__ice_render_source_location);
let __ice_rendered: __IceElement<'_, __DucktapeMessage> = { let __mouse_content: __IceElement<'_, __DucktapeMessage> = {
#[cfg(test)]
let __ice_render_source_location = ::ui_lang_runtime::testing::Location::new("components/onboarding.ice", 65, 1, "rendered view node");
#[cfg(test)]
let __ice_render_source = ::ui_lang_runtime::testing::push_render_source(__ice_render_source_location);
let __ice_rendered: __IceElement<'_, __DucktapeMessage> = ::iced::widget::space().width(::iced::Fill).height(34.0 as f32).into();
#[cfg(test)]
drop(__ice_render_source);
__ice_rendered
}; ::iced::widget::mouse_area(__mouse_content).on_press((move || __DucktapeMessage::DragLaunchWindow)()).into() };
#[cfg(test)]
drop(__ice_render_source);
__ice_rendered
}); __children.push({
#[cfg(test)]
let __ice_render_source_location = ::ui_lang_runtime::testing::Location::new("components/onboarding.ice", 66, 1, "rendered view node");
#[cfg(test)]
let __ice_render_source = ::ui_lang_runtime::testing::push_render_source(__ice_render_source_location);
let __ice_rendered: __IceElement<'_, __DucktapeMessage> = { let __ice_node_scope = format!("{}/launch-close", __ice_node_scope); { let __a11y_key = __ice_node_scope.clone(); let __a11y_id = ::ui_lang_runtime::StableId::new(&__a11y_key); let __disabled = false; let __activate = (move || __DucktapeMessage::CloseLaunchWindow)(); let __button_content: __IceElement<'_, __DucktapeMessage> = {
#[cfg(test)]
let __ice_render_source_location = ::ui_lang_runtime::testing::Location::new("components/onboarding.ice", 74, 1, "rendered view node");
#[cfg(test)]
let __ice_render_source = ::ui_lang_runtime::testing::push_render_source(__ice_render_source_location);
let __ice_rendered: __IceElement<'_, __DucktapeMessage> = { let __a11y_key = format!("{}/@text:74", __ice_component_0487562436f6c756d6e_scope_10219); let __text_value = ("✕").to_string(); let __text = ::iced::widget::text(__text_value.clone()).size(((12.5) as f32).max(f32::EPSILON).min(f32::MAX)).font(::iced::Font { family: ::iced::font::Family::Name("Geist"), weight: ::iced::font::Weight::Normal, stretch: ::iced::font::Stretch::Normal, style: ::iced::font::Style::Normal }); let __text = ::ui_lang_runtime::selectable_text(__text); ::ui_lang_runtime::accessible(__text, ::ui_lang_runtime::StableId::new(&__a11y_key), ::ui_lang_runtime::Role::Label).logical_id_maybe(::core::cfg!(test).then_some(__a11y_key)).value(__text_value).into() };
#[cfg(test)]
drop(__ice_render_source);
__ice_rendered
}; let __button = ::iced::widget::button(__button_content).padding(::iced::Padding { top: 5.0, right: 5.0, bottom: 5.0, left: 5.0 }).on_press_maybe(if __disabled { None } else { Some(__activate.clone()) }).style(move |__theme, __status| { let mut __style = ::iced::widget::button::primary(__theme, __status); let __background: Option<::iced::Color> = match __status { ::iced::widget::button::Status::Hovered => Some(__ice_palette.colors[14]), ::iced::widget::button::Status::Pressed => Some(__ice_palette.colors[39]), ::iced::widget::button::Status::Disabled => Some(::iced::Color::from_rgba8(0, 0, 0, 0.000000)), _ => Some(::iced::Color::from_rgba8(0, 0, 0, 0.000000)) }; if let Some(__background) = __background { __style.background = Some(::iced::Background::Color(__background)); } __style.text_color = __ice_palette.colors[4]; __style.border.radius = 7.0.into(); __style.background = ::std::option::Option::Some(::iced::Background::Color(::iced::Color::from_rgba8(0, 0, 0, 0.000000))); __style.text_color = __ice_palette.colors[5]; __style.border.color = ::iced::Color::from_rgba8(0, 0, 0, 0.000000); __style.border.width = 1.0 as f32; __style.border.radius = ::iced::border::Radius { top_left: ((6.0) as f32).max(0.0).min(f32::MAX), top_right: ((6.0) as f32).max(0.0).min(f32::MAX), bottom_right: ((6.0) as f32).max(0.0).min(f32::MAX), bottom_left: ((6.0) as f32).max(0.0).min(f32::MAX) }; match __status { ::iced::widget::button::Status::Hovered => { __style.background = ::std::option::Option::Some(::iced::Background::Color(__ice_palette.colors[56])); __style.text_color = __ice_palette.colors[4]; } ::iced::widget::button::Status::Pressed => { __style.background = ::std::option::Option::Some(::iced::Background::Color(__ice_palette.colors[56])); __style.text_color = __ice_palette.colors[4]; } _ => {} } if matches!(__status, ::iced::widget::button::Status::Disabled) { if let Some(::iced::Background::Color(mut __color)) = __style.background { __color.a *= 0.5; __style.background = Some(::iced::Background::Color(__color)); } __style.text_color.a *= 0.5; } __style }); ::ui_lang_runtime::accessible(__button, __a11y_id, ::ui_lang_runtime::Role::Button).logical_id_maybe(::core::cfg!(test).then_some(&*__a11y_key)).focus_id(::iced::widget::Id::from(__a11y_key)).label("Close Ducktape".to_owned()).disabled(__disabled).on_activate_maybe(if __disabled { None } else { Some(__activate) }).focus_ring(__ice_palette.colors[42], 7.0).into() } };
#[cfg(test)]
drop(__ice_render_source);
__ice_rendered
}); let __child_count = __children.len(); let __children = __children.into_iter().map(|__child| ::ui_lang_runtime::bounded_fill_element(__child, __child_count, true)).collect::<::std::vec::Vec<_>>(); let __layout = ::iced::widget::row(__children).padding(::ui_lang_runtime::bounded_padding(0.0, 10.0, 0.0, 10.0)).width(::iced::Fill).height(34.0 as f32).align_y(::iced::alignment::Vertical::Center); let __content = ::iced::widget::container(__layout); let __layout_content: __IceElement<'_, __DucktapeMessage> = __content.into(); let __a11y_key = format!("{}/@layout:58", __ice_component_0487562436f6c756d6e_scope_10219); ::ui_lang_runtime::accessible(__layout_content, ::ui_lang_runtime::StableId::new(&__a11y_key), ::ui_lang_runtime::Role::GenericContainer).logical_id_maybe(::core::cfg!(test).then_some(__a11y_key)).into() };
#[cfg(test)]
drop(__ice_render_source);
__ice_rendered
}); __children.push({
#[cfg(test)]
let __ice_render_source_location = ::ui_lang_runtime::testing::Location::new("components/onboarding.ice", 75, 1, "rendered view node");
#[cfg(test)]
let __ice_render_source = ::ui_lang_runtime::testing::push_render_source(__ice_render_source_location);
let __ice_rendered: __IceElement<'_, __DucktapeMessage> = { let __a11y_key = format!("{}/@container:75", __ice_component_0487562436f6c756d6e_scope_10219); let __container_content: __IceElement<'_, __DucktapeMessage> = {
#[cfg(test)]
let __ice_render_source_location = ::ui_lang_runtime::testing::Location::new("components/onboarding.ice", 84, 1, "rendered view node");
#[cfg(test)]
let __ice_render_source = ::ui_lang_runtime::testing::push_render_source(__ice_render_source_location);
let __ice_rendered: __IceElement<'_, __DucktapeMessage> = { let mut __children: ::std::vec::Vec<__IceElement<'_, __DucktapeMessage>> = ::std::vec::Vec::new(); match &(self.hub_step) { HubStep::Wallets => { __children.push({
#[cfg(test)]
let __ice_render_source_location = ::ui_lang_runtime::testing::Location::new("components/onboarding.ice", 87, 1, "rendered view node");
#[cfg(test)]
let __ice_render_source = ::ui_lang_runtime::testing::push_render_source(__ice_render_source_location);
let __ice_rendered: __IceElement<'_, __DucktapeMessage> = { let __ice_node_scope = format!("{}/wallets", __ice_node_scope); ::ui_lang_runtime::grow_stack(|| self.__ice_component_use_6(__ice_palette, __ice_node_scope.clone(), (move || __DucktapeMessage::GoNetworks).clone(), (move || __DucktapeMessage::GoRestore).clone(), (move || __DucktapeMessage::LoginSkip).clone(), (move |__event_0| __DucktapeMessage::PickWallet(__event_0)).clone(), (move |__event_0| __DucktapeMessage::UnlockSubmit(__event_0)).clone())) };
#[cfg(test)]
drop(__ice_render_source);
__ice_rendered
}); }, HubStep::Password => { __children.push({
#[cfg(test)]
let __ice_render_source_location = ::ui_lang_runtime::testing::Location::new("components/onboarding.ice", 101, 1, "rendered view node");
#[cfg(test)]
let __ice_render_source = ::ui_lang_runtime::testing::push_render_source(__ice_render_source_location);
let __ice_rendered: __IceElement<'_, __DucktapeMessage> = { let __ice_node_scope = format!("{}/password", __ice_node_scope); { let __ice_component_050617373776f726453637265656e_scope_3952 = __ice_node_scope.clone(); ::ui_lang_runtime::rev_memo(502u64, (self.__ice_component_050617373776f726453637265656e.get(&__ice_component_050617373776f726453637265656e_scope_3952).map_or(0, |__state| __state.__ice_rev[0]), self.__ice_component_050617373776f726453637265656e.get(&__ice_component_050617373776f726453637265656e_scope_3952).map_or(0, |__state| __state.__ice_rev[1]), self.__ice_rev[124], self.__ice_rev[131], self.__ice_rev[132], self.__ice_rev[19], __ice_palette.name), ::ui_lang_runtime::grow_stack(|| self.__ice_component_use_8(__ice_palette, __ice_component_050617373776f726453637265656e_scope_3952, (move || __DucktapeMessage::GoNetworks).clone(), (move || __DucktapeMessage::GoRestore).clone(), (move || __DucktapeMessage::LoginSkip).clone(), (move |__event_0| __DucktapeMessage::PasswordSubmit(__event_0)).clone()))).into() } };
#[cfg(test)]
drop(__ice_render_source);
__ice_rendered
}); }, HubStep::Phrase => { __children.push({
#[cfg(test)]
let __ice_render_source_location = ::ui_lang_runtime::testing::Location::new("components/onboarding.ice", 111, 1, "rendered view node");
#[cfg(test)]
let __ice_render_source = ::ui_lang_runtime::testing::push_render_source(__ice_render_source_location);
let __ice_rendered: __IceElement<'_, __DucktapeMessage> = { let __ice_node_scope = format!("{}/phrase", __ice_node_scope); { let __ice_component_050687261736553637265656e_scope_3962 = __ice_node_scope.clone(); ::ui_lang_runtime::rev_memo(503u64, (self.__ice_rev[131], self.__ice_rev[132], self.__ice_rev[19], __ice_palette.name), ::ui_lang_runtime::grow_stack(|| self.__ice_component_use_10(__ice_palette, __ice_component_050687261736553637265656e_scope_3962, (move || __DucktapeMessage::PhraseWrittenDown).clone()))).into() } };
#[cfg(test)]
drop(__ice_render_source);
__ice_rendered
}); }, HubStep::Confirm => { __children.push({
#[cfg(test)]
let __ice_render_source_location = ::ui_lang_runtime::testing::Location::new("components/onboarding.ice", 115, 1, "rendered view node");
#[cfg(test)]
let __ice_render_source = ::ui_lang_runtime::testing::push_render_source(__ice_render_source_location);
let __ice_rendered: __IceElement<'_, __DucktapeMessage> = { let __ice_node_scope = format!("{}/confirm", __ice_node_scope); { let __ice_component_0436f6e6669726d50687261736553637265656e_scope_3966 = __ice_node_scope.clone(); ::ui_lang_runtime::rev_memo(504u64, (self.__ice_component_0436f6e6669726d50687261736553637265656e.get(&__ice_component_0436f6e6669726d50687261736553637265656e_scope_3966).map_or(0, |__state| __state.__ice_rev[0]), self.__ice_rev[131], self.__ice_rev[132], self.__ice_rev[19], __ice_palette.name), ::ui_lang_runtime::grow_stack(|| self.__ice_component_use_11(__ice_palette, __ice_component_0436f6e6669726d50687261736553637265656e_scope_3966, (move |__event_0| __DucktapeMessage::ConfirmPhraseSubmit(__event_0)).clone(), (move || __DucktapeMessage::ShowPhraseAgain).clone()))).into() } };
#[cfg(test)]
drop(__ice_render_source);
__ice_rendered
}); }, HubStep::Restore => { __children.push({
#[cfg(test)]
let __ice_render_source_location = ::ui_lang_runtime::testing::Location::new("components/onboarding.ice", 124, 1, "rendered view node");
#[cfg(test)]
let __ice_render_source = ::ui_lang_runtime::testing::push_render_source(__ice_render_source_location);
let __ice_rendered: __IceElement<'_, __DucktapeMessage> = { let __ice_component_0526573746f726553637265656e_scope_3975 = format!("{}/RestoreScreen@3975", __ice_component_0487562436f6c756d6e_scope_10219); let __ice_cb_0 = (move || __DucktapeMessage::CloseLaunchWindow).clone(); let __ice_cb_1 = (move |__event_0| __DucktapeMessage::ConfirmPhraseSubmit(__event_0)).clone(); let __ice_cb_2 = (move |__event_0| __DucktapeMessage::ConnectRemoteSubmit(__event_0)).clone(); let __ice_cb_3 = (move || __DucktapeMessage::CopyOnboardingInvite).clone(); let __ice_cb_4 = (move || __DucktapeMessage::DragLaunchWindow).clone(); let __ice_cb_5 = (move || __DucktapeMessage::EnterConsole).clone(); let __ice_cb_6 = (move |__event_0| __DucktapeMessage::ForgetNetworkSubmit(__event_0)).clone(); let __ice_cb_7 = (move || __DucktapeMessage::GoJoin).clone(); let __ice_cb_8 = (move || __DucktapeMessage::GoLogin).clone(); let __ice_cb_9 = (move || __DucktapeMessage::GoNetworks).clone(); let __ice_cb_10 = (move || __DucktapeMessage::GoRestore).clone(); let __ice_cb_11 = (move || __DucktapeMessage::JoinNetworkSubmit).clone(); let __ice_cb_12 = (move || __DucktapeMessage::LoginSkip).clone(); let __ice_cb_13 = (move || __DucktapeMessage::OpenNetworkSubmit).clone(); let __ice_cb_14 = (move |__event_0| __DucktapeMessage::PasswordSubmit(__event_0)).clone(); let __ice_cb_15 = (move || __DucktapeMessage::PhraseWrittenDown).clone(); let __ice_cb_16 = (move |__event_0| __DucktapeMessage::PickNetwork(__event_0)).clone(); let __ice_cb_17 = (move |__event_0| __DucktapeMessage::PickWallet(__event_0)).clone(); let __ice_cb_18 = (move |__event_0, __event_1| __DucktapeMessage::RestoreSubmit(__event_0, __event_1)).clone(); let __ice_cb_19 = (move || __DucktapeMessage::ShowPhraseAgain).clone(); let __ice_cb_20 = (move |__event_0| __DucktapeMessage::UnlockSubmit(__event_0)).clone(); let __ice_cb_21 = (move || __DucktapeMessage::WelcomeCancel).clone(); let __ice_cb_22 = (move |__event_0| __DucktapeMessage::WelcomeCreateSubmit(__event_0)).clone(); let __ice_cb_23 = (move || __DucktapeMessage::WelcomeDesktop).clone(); let __ice_cb_24 = (move || __DucktapeMessage::WelcomeLoginSubmit).clone(); let __ice_cb_25 = (move || __DucktapeMessage::WelcomeSkip).clone(); #[cfg(test)] ::ui_lang_runtime::testing::register_component_sighting("RestoreScreen", &__ice_component_0526573746f726553637265656e_scope_3975); let __component_content: __IceElement<'_, __DucktapeMessage> = {
#[cfg(test)]
let __ice_render_source_location = ::ui_lang_runtime::testing::Location::new("components/onboarding.ice", 991, 1, "rendered view node");
#[cfg(test)]
let __ice_render_source = ::ui_lang_runtime::testing::push_render_source(__ice_render_source_location);
let __ice_rendered: __IceElement<'_, __DucktapeMessage> = { let __ice_node_scope = format!("{}/root", __ice_component_0526573746f726553637265656e_scope_3975); { let mut __children: ::std::vec::Vec<__IceElement<'_, __DucktapeMessage>> = ::std::vec::Vec::new(); __children.push({
#[cfg(test)]
let __ice_render_source_location = ::ui_lang_runtime::testing::Location::new("components/onboarding.ice", 992, 1, "rendered view node");
#[cfg(test)]
let __ice_render_source = ::ui_lang_runtime::testing::push_render_source(__ice_render_source_location);
let __ice_rendered: __IceElement<'_, __DucktapeMessage> = { let __a11y_key = format!("{}/@button:992", __ice_component_0526573746f726553637265656e_scope_3975); let __a11y_id = ::ui_lang_runtime::StableId::new(&__a11y_key); let __disabled = false; let __activate = (__ice_cb_8)(); let __button_content: __IceElement<'_, __DucktapeMessage> = {
#[cfg(test)]
let __ice_render_source_location = ::ui_lang_runtime::testing::Location::new("components/onboarding.ice", 999, 1, "rendered view node");
#[cfg(test)]
let __ice_render_source = ::ui_lang_runtime::testing::push_render_source(__ice_render_source_location);
let __ice_rendered: __IceElement<'_, __DucktapeMessage> = { let mut __children: ::std::vec::Vec<__IceElement<'_, __DucktapeMessage>> = ::std::vec::Vec::new(); __children.push({
#[cfg(test)]
let __ice_render_source_location = ::ui_lang_runtime::testing::Location::new("components/onboarding.ice", 1000, 1, "rendered view node");
#[cfg(test)]
let __ice_render_source = ::ui_lang_runtime::testing::push_render_source(__ice_render_source_location);
let __ice_rendered: __IceElement<'_, __DucktapeMessage> = { let __a11y_key = format!("{}/@text:1000", __ice_component_0526573746f726553637265656e_scope_3975); let __text_value = ("‹").to_string(); let __text = ::iced::widget::text(__text_value.clone()).size(((14.0) as f32).max(f32::EPSILON).min(f32::MAX)).wrapping(::iced::widget::text::Wrapping::None).color(__ice_palette.colors[71]); let __text = ::ui_lang_runtime::selectable_text(__text); ::ui_lang_runtime::accessible(__text, ::ui_lang_runtime::StableId::new(&__a11y_key), ::ui_lang_runtime::Role::Label).logical_id_maybe(::core::cfg!(test).then_some(__a11y_key)).value(__text_value).into() };
#[cfg(test)]
drop(__ice_render_source);
__ice_rendered
}); __children.push({
#[cfg(test)]
let __ice_render_source_location = ::ui_lang_runtime::testing::Location::new("components/onboarding.ice", 1005, 1, "rendered view node");
#[cfg(test)]
let __ice_render_source = ::ui_lang_runtime::testing::push_render_source(__ice_render_source_location);
let __ice_rendered: __IceElement<'_, __DucktapeMessage> = { let __a11y_key = format!("{}/@text:1005", __ice_component_0526573746f726553637265656e_scope_3975); let __text_value = ("BACK").to_string(); let __text = ::iced::widget::text(__text_value.clone()).size(((11.0) as f32).max(f32::EPSILON).min(f32::MAX)).wrapping(::iced::widget::text::Wrapping::None).font(::iced::Font { family: ::iced::font::Family::Name("Geist Mono"), weight: ::iced::font::Weight::Medium, stretch: ::iced::font::Stretch::Normal, style: ::iced::font::Style::Normal }).color(__ice_palette.colors[71]); let __text = ::ui_lang_runtime::selectable_text(__text); ::ui_lang_runtime::accessible(__text, ::ui_lang_runtime::StableId::new(&__a11y_key), ::ui_lang_runtime::Role::Label).logical_id_maybe(::core::cfg!(test).then_some(__a11y_key)).value(__text_value).into() };
#[cfg(test)]
drop(__ice_render_source);
__ice_rendered
}); let __child_count = __children.len(); let __children = __children.into_iter().map(|__child| ::ui_lang_runtime::bounded_fill_element(__child, __child_count, true)).collect::<::std::vec::Vec<_>>(); let __layout = ::iced::widget::row(__children).spacing(::ui_lang_runtime::bounded_spacing(8.0, __child_count)).align_y(::iced::alignment::Vertical::Center); let __content = ::iced::widget::container(__layout); let __layout_content: __IceElement<'_, __DucktapeMessage> = __content.into(); let __a11y_key = format!("{}/@layout:999", __ice_component_0526573746f726553637265656e_scope_3975); ::ui_lang_runtime::accessible(__layout_content, ::ui_lang_runtime::StableId::new(&__a11y_key), ::ui_lang_runtime::Role::GenericContainer).logical_id_maybe(::core::cfg!(test).then_some(__a11y_key)).into() };
#[cfg(test)]
drop(__ice_render_source);
__ice_rendered
}; let __button = ::iced::widget::button(__button_content).on_press_maybe(if __disabled { None } else { Some(__activate.clone()) }).style(move |__theme, __status| { let mut __style = ::iced::widget::button::primary(__theme, __status); let __background: Option<::iced::Color> = match __status { ::iced::widget::button::Status::Hovered => Some(__ice_palette.colors[14]), ::iced::widget::button::Status::Pressed => Some(__ice_palette.colors[39]), ::iced::widget::button::Status::Disabled => Some(::iced::Color::from_rgba8(0, 0, 0, 0.000000)), _ => Some(::iced::Color::from_rgba8(0, 0, 0, 0.000000)) }; if let Some(__background) = __background { __style.background = Some(::iced::Background::Color(__background)); } __style.text_color = __ice_palette.colors[4]; __style.border.radius = 6.0.into(); if matches!(__status, ::iced::widget::button::Status::Disabled) { if let Some(::iced::Background::Color(mut __color)) = __style.background { __color.a *= 0.5; __style.background = Some(::iced::Background::Color(__color)); } __style.text_color.a *= 0.5; } __style }); ::ui_lang_runtime::accessible(__button, __a11y_id, ::ui_lang_runtime::Role::Button).logical_id_maybe(::core::cfg!(test).then_some(&*__a11y_key)).focus_id(::iced::widget::Id::from(__a11y_key)).label("Back".to_owned()).disabled(__disabled).on_activate_maybe(if __disabled { None } else { Some(__activate) }).focus_ring(__ice_palette.colors[42], 6.0).into() };
#[cfg(test)]
drop(__ice_render_source);
__ice_rendered
}); __children.push({
#[cfg(test)]
let __ice_render_source_location = ::ui_lang_runtime::testing::Location::new("components/onboarding.ice", 1011, 1, "rendered view node");
#[cfg(test)]
let __ice_render_source = ::ui_lang_runtime::testing::push_render_source(__ice_render_source_location);
let __ice_rendered: __IceElement<'_, __DucktapeMessage> = { let __a11y_key = format!("{}/@container:1011", __ice_component_0526573746f726553637265656e_scope_3975); let __container_content: __IceElement<'_, __DucktapeMessage> = {
#[cfg(test)]
let __ice_render_source_location = ::ui_lang_runtime::testing::Location::new("components/onboarding.ice", 1012, 1, "rendered view node");
#[cfg(test)]
let __ice_render_source = ::ui_lang_runtime::testing::push_render_source(__ice_render_source_location);
let __ice_rendered: __IceElement<'_, __DucktapeMessage> = { let __a11y_key = format!("{}/@text:1012", __ice_component_0526573746f726553637265656e_scope_3975); let __text_value = ("Restore your identity").to_string(); let __text = ::iced::widget::text(__text_value.clone()).size(((20.0) as f32).max(f32::EPSILON).min(f32::MAX)).width(::iced::Fill).wrapping(::iced::widget::text::Wrapping::None).font(::iced::Font { family: ::iced::font::Family::Name("Geist"), weight: ::iced::font::Weight::Semibold, stretch: ::iced::font::Stretch::Normal, style: ::iced::font::Style::Normal }).color(__ice_palette.colors[7]); let __text = ::ui_lang_runtime::selectable_text(__text); ::ui_lang_runtime::accessible(__text, ::ui_lang_runtime::StableId::new(&__a11y_key), ::ui_lang_runtime::Role::Label).logical_id_maybe(::core::cfg!(test).then_some(__a11y_key)).value(__text_value).into() };
#[cfg(test)]
drop(__ice_render_source);
__ice_rendered
}; let __container = {  ::iced::widget::container(__container_content).padding(::ui_lang_runtime::bounded_padding(16.0, 0.0, 0.0, 0.0)).width(::iced::Fill) }; ::ui_lang_runtime::accessible(__container, ::ui_lang_runtime::StableId::new(&__a11y_key), ::ui_lang_runtime::Role::GenericContainer).logical_id_maybe(::core::cfg!(test).then_some(__a11y_key)).into() };
#[cfg(test)]
drop(__ice_render_source);
__ice_rendered
}); __children.push({
#[cfg(test)]
let __ice_render_source_location = ::ui_lang_runtime::testing::Location::new("components/onboarding.ice", 1019, 1, "rendered view node");
#[cfg(test)]
let __ice_render_source = ::ui_lang_runtime::testing::push_render_source(__ice_render_source_location);
let __ice_rendered: __IceElement<'_, __DucktapeMessage> = { let __a11y_key = format!("{}/@container:1019", __ice_component_0526573746f726553637265656e_scope_3975); let __container_content: __IceElement<'_, __DucktapeMessage> = {
#[cfg(test)]
let __ice_render_source_location = ::ui_lang_runtime::testing::Location::new("components/onboarding.ice", 1020, 1, "rendered view node");
#[cfg(test)]
let __ice_render_source = ::ui_lang_runtime::testing::push_render_source(__ice_render_source_location);
let __ice_rendered: __IceElement<'_, __DucktapeMessage> = { let __a11y_key = format!("{}/@text:1020", __ice_component_0526573746f726553637265656e_scope_3975); let __text_value = ("Paste the 24 recovery words, pick a new password for this device.").to_string(); let __text = ::iced::widget::text(__text_value.clone()).size(((13.0) as f32).max(f32::EPSILON).min(f32::MAX)).width(::iced::Fill).line_height(::iced::widget::text::LineHeight::Relative(((1.5) as f32).max(f32::EPSILON).min(f32::MAX))).color(__ice_palette.colors[70]); let __text = ::ui_lang_runtime::selectable_text(__text); ::ui_lang_runtime::accessible(__text, ::ui_lang_runtime::StableId::new(&__a11y_key), ::ui_lang_runtime::Role::Label).logical_id_maybe(::core::cfg!(test).then_some(__a11y_key)).value(__text_value).into() };
#[cfg(test)]
drop(__ice_render_source);
__ice_rendered
}; let __container = {  ::iced::widget::container(__container_content).padding(::ui_lang_runtime::bounded_padding(6.0, 0.0, 0.0, 0.0)).width(::iced::Fill) }; ::ui_lang_runtime::accessible(__container, ::ui_lang_runtime::StableId::new(&__a11y_key), ::ui_lang_runtime::Role::GenericContainer).logical_id_maybe(::core::cfg!(test).then_some(__a11y_key)).into() };
#[cfg(test)]
drop(__ice_render_source);
__ice_rendered
}); __children.push({
#[cfg(test)]
let __ice_render_source_location = ::ui_lang_runtime::testing::Location::new("components/onboarding.ice", 1026, 1, "rendered view node");
#[cfg(test)]
let __ice_render_source = ::ui_lang_runtime::testing::push_render_source(__ice_render_source_location);
let __ice_rendered: __IceElement<'_, __DucktapeMessage> = { let __a11y_key = format!("{}/@container:1026", __ice_component_0526573746f726553637265656e_scope_3975); let __container_content: __IceElement<'_, __DucktapeMessage> = {
#[cfg(test)]
let __ice_render_source_location = ::ui_lang_runtime::testing::Location::new("components/onboarding.ice", 1027, 1, "rendered view node");
#[cfg(test)]
let __ice_render_source = ::ui_lang_runtime::testing::push_render_source(__ice_render_source_location);
let __ice_rendered: __IceElement<'_, __DucktapeMessage> = { let __a11y_key = format!("{}/@text:1027", __ice_component_0526573746f726553637265656e_scope_3975); let __text_value = ("WALLET NAME").to_string(); let __text = ::iced::widget::text(__text_value.clone()).size(((10.0) as f32).max(f32::EPSILON).min(f32::MAX)).wrapping(::iced::widget::text::Wrapping::None).font(::iced::Font { family: ::iced::font::Family::Name("Geist Mono"), weight: ::iced::font::Weight::Semibold, stretch: ::iced::font::Stretch::Normal, style: ::iced::font::Style::Normal }).color(__ice_palette.colors[73]); let __text = ::ui_lang_runtime::selectable_text(__text); ::ui_lang_runtime::accessible(__text, ::ui_lang_runtime::StableId::new(&__a11y_key), ::ui_lang_runtime::Role::Label).logical_id_maybe(::core::cfg!(test).then_some(__a11y_key)).value(__text_value).into() };
#[cfg(test)]
drop(__ice_render_source);
__ice_rendered
}; let __container = {  ::iced::widget::container(__container_content).padding(::ui_lang_runtime::bounded_padding(20.0, 0.0, 0.0, 0.0)).width(::iced::Fill) }; ::ui_lang_runtime::accessible(__container, ::ui_lang_runtime::StableId::new(&__a11y_key), ::ui_lang_runtime::Role::GenericContainer).logical_id_maybe(::core::cfg!(test).then_some(__a11y_key)).into() };
#[cfg(test)]
drop(__ice_render_source);
__ice_rendered
}); __children.push({
#[cfg(test)]
let __ice_render_source_location = ::ui_lang_runtime::testing::Location::new("components/onboarding.ice", 1033, 1, "rendered view node");
#[cfg(test)]
let __ice_render_source = ::ui_lang_runtime::testing::push_render_source(__ice_render_source_location);
let __ice_rendered: __IceElement<'_, __DucktapeMessage> = { let __a11y_key = format!("{}/@container:1033", __ice_component_0526573746f726553637265656e_scope_3975); let __container_content: __IceElement<'_, __DucktapeMessage> = {
#[cfg(test)]
let __ice_render_source_location = ::ui_lang_runtime::testing::Location::new("components/onboarding.ice", 1034, 1, "rendered view node");
#[cfg(test)]
let __ice_render_source = ::ui_lang_runtime::testing::push_render_source(__ice_render_source_location);
let __ice_rendered: __IceElement<'_, __DucktapeMessage> = { let __a11y_key = format!("{}/@container:1034", __ice_component_0526573746f726553637265656e_scope_3975); let __container_content: __IceElement<'_, __DucktapeMessage> = {
#[cfg(test)]
let __ice_render_source_location = ::ui_lang_runtime::testing::Location::new("components/onboarding.ice", 1043, 1, "rendered view node");
#[cfg(test)]
let __ice_render_source = ::ui_lang_runtime::testing::push_render_source(__ice_render_source_location);
let __ice_rendered: __IceElement<'_, __DucktapeMessage> = { let __ice_node_scope = format!("{}/restore-name", __ice_node_scope); { let __a11y_key = __ice_node_scope.clone(); let __a11y_id = ::ui_lang_runtime::StableId::new(&__a11y_key); let __disabled = (*self.__ice_derived_hub_busy()); let __secure = false; let __role = if __secure { ::ui_lang_runtime::Role::PasswordInput } else { ::ui_lang_runtime::Role::TextInput }; let __input = ::ui_lang_runtime::accessible(::iced::widget::text_input("lowercase letters, digits, . _ -", &self.__ice_component_0526573746f726553637265656e.get(&__ice_component_0526573746f726553637265656e_scope_3975).map_or_else(|| "restored".to_owned(), |__state| __state.name_draft.clone())).id(::iced::widget::Id::from(__a11y_key.clone())).padding(::iced::Padding { top: 11.0, right: 13.0, bottom: 11.0, left: 13.0 }).width(::iced::Fill).secure(__secure).width(::iced::Fill).padding(0.0 as f32).size(((13.0) as f32).max(f32::EPSILON).min(f32::MAX)).line_height(::iced::widget::text::LineHeight::Relative(((1.2) as f32).max(f32::EPSILON).min(f32::MAX))).font(::iced::Font { family: ::iced::font::Family::Name("Geist Mono"), weight: ::iced::font::Weight::Normal, stretch: ::iced::font::Stretch::Normal, style: ::iced::font::Style::Normal }).on_input_maybe(if __disabled { None } else { Some({ let __scope = (__ice_component_0526573746f726553637265656e_scope_3975).clone(); move |__value| __DucktapeMessage::__0C526573746f726553637265656eB6e616d655f6472616674(__scope.clone(), __value) }) }).style(move |__theme, __status| { let mut __style = ::iced::widget::text_input::default(__theme, __status); __style.background = __ice_palette.colors[3].into(); __style.border.color = __ice_palette.colors[39]; __style.border.width = 1.0; __style.border.radius = 10.0.into(); __style.background = ::iced::Background::Color(::iced::Color::from_rgba8(0, 0, 0, 0.000000)); __style.border.color = ::iced::Color::from_rgba8(0, 0, 0, 0.000000); __style.border.width = 0.0 as f32; __style.border.radius = ::iced::border::Radius { top_left: ((0.0) as f32).max(0.0).min(f32::MAX), top_right: ((0.0) as f32).max(0.0).min(f32::MAX), bottom_right: ((0.0) as f32).max(0.0).min(f32::MAX), bottom_left: ((0.0) as f32).max(0.0).min(f32::MAX) }; __style.placeholder = __ice_palette.colors[73]; __style.value = __ice_palette.colors[4]; __style.selection = { let mut __color = __ice_palette.colors[4]; __color.a = 0.180000; __color }; if matches!(__status, ::iced::widget::text_input::Status::Focused { .. }) { __style.border.color = __ice_palette.colors[42]; } match __status { ::iced::widget::text_input::Status::Disabled => { __style.value = __ice_palette.colors[72]; } _ => {} } __style }), __a11y_id, __role).logical_id_maybe(::core::cfg!(test).then_some(&*__a11y_key)).focus_id(::iced::widget::Id::from(__a11y_key)).label("Wallet name".to_owned()).value_maybe((!__secure).then(|| (self.__ice_component_0526573746f726553637265656e.get(&__ice_component_0526573746f726553637265656e_scope_3975).map_or_else(|| "restored".to_owned(), |__state| __state.name_draft.clone())).to_owned())).disabled(__disabled); __input.into() } };
#[cfg(test)]
drop(__ice_render_source);
__ice_rendered
}; let __container = {  ::iced::widget::container(__container_content).padding(::ui_lang_runtime::bounded_padding(12.0, 14.0, 12.0, 14.0)).width(::iced::Fill).style(move |__theme| { let mut __style = ::iced::widget::container::Style::default(); __style.background = ::std::option::Option::Some(::iced::Background::Color(__ice_palette.colors[3])); __style.border.color = __ice_palette.colors[7]; __style.border.width = 1.5 as f32; __style.border.radius = ::iced::border::Radius { top_left: ((10.0) as f32).max(0.0).min(f32::MAX), top_right: ((10.0) as f32).max(0.0).min(f32::MAX), bottom_right: ((10.0) as f32).max(0.0).min(f32::MAX), bottom_left: ((10.0) as f32).max(0.0).min(f32::MAX) }; __style }) }; ::ui_lang_runtime::accessible(__container, ::ui_lang_runtime::StableId::new(&__a11y_key), ::ui_lang_runtime::Role::GenericContainer).logical_id_maybe(::core::cfg!(test).then_some(__a11y_key)).into() };
#[cfg(test)]
drop(__ice_render_source);
__ice_rendered
}; let __container = {  ::iced::widget::container(__container_content).padding(::ui_lang_runtime::bounded_padding(8.0, 0.0, 0.0, 0.0)).width(::iced::Fill) }; ::ui_lang_runtime::accessible(__container, ::ui_lang_runtime::StableId::new(&__a11y_key), ::ui_lang_runtime::Role::GenericContainer).logical_id_maybe(::core::cfg!(test).then_some(__a11y_key)).into() };
#[cfg(test)]
drop(__ice_render_source);
__ice_rendered
}); __children.push({
#[cfg(test)]
let __ice_render_source_location = ::ui_lang_runtime::testing::Location::new("components/onboarding.ice", 1056, 1, "rendered view node");
#[cfg(test)]
let __ice_render_source = ::ui_lang_runtime::testing::push_render_source(__ice_render_source_location);
let __ice_rendered: __IceElement<'_, __DucktapeMessage> = { let __a11y_key = format!("{}/@container:1056", __ice_component_0526573746f726553637265656e_scope_3975); let __container_content: __IceElement<'_, __DucktapeMessage> = {
#[cfg(test)]
let __ice_render_source_location = ::ui_lang_runtime::testing::Location::new("components/onboarding.ice", 1057, 1, "rendered view node");
#[cfg(test)]
let __ice_render_source = ::ui_lang_runtime::testing::push_render_source(__ice_render_source_location);
let __ice_rendered: __IceElement<'_, __DucktapeMessage> = { let __a11y_key = format!("{}/@text:1057", __ice_component_0526573746f726553637265656e_scope_3975); let __text_value = ("RECOVERY PHRASE").to_string(); let __text = ::iced::widget::text(__text_value.clone()).size(((10.0) as f32).max(f32::EPSILON).min(f32::MAX)).wrapping(::iced::widget::text::Wrapping::None).font(::iced::Font { family: ::iced::font::Family::Name("Geist Mono"), weight: ::iced::font::Weight::Semibold, stretch: ::iced::font::Stretch::Normal, style: ::iced::font::Style::Normal }).color(__ice_palette.colors[73]); let __text = ::ui_lang_runtime::selectable_text(__text); ::ui_lang_runtime::accessible(__text, ::ui_lang_runtime::StableId::new(&__a11y_key), ::ui_lang_runtime::Role::Label).logical_id_maybe(::core::cfg!(test).then_some(__a11y_key)).value(__text_value).into() };
#[cfg(test)]
drop(__ice_render_source);
__ice_rendered
}; let __container = {  ::iced::widget::container(__container_content).padding(::ui_lang_runtime::bounded_padding(14.0, 0.0, 0.0, 0.0)).width(::iced::Fill) }; ::ui_lang_runtime::accessible(__container, ::ui_lang_runtime::StableId::new(&__a11y_key), ::ui_lang_runtime::Role::GenericContainer).logical_id_maybe(::core::cfg!(test).then_some(__a11y_key)).into() };
#[cfg(test)]
drop(__ice_render_source);
__ice_rendered
}); __children.push({
#[cfg(test)]
let __ice_render_source_location = ::ui_lang_runtime::testing::Location::new("components/onboarding.ice", 1063, 1, "rendered view node");
#[cfg(test)]
let __ice_render_source = ::ui_lang_runtime::testing::push_render_source(__ice_render_source_location);
let __ice_rendered: __IceElement<'_, __DucktapeMessage> = { let __a11y_key = format!("{}/@container:1063", __ice_component_0526573746f726553637265656e_scope_3975); let __container_content: __IceElement<'_, __DucktapeMessage> = {
#[cfg(test)]
let __ice_render_source_location = ::ui_lang_runtime::testing::Location::new("components/onboarding.ice", 1064, 1, "rendered view node");
#[cfg(test)]
let __ice_render_source = ::ui_lang_runtime::testing::push_render_source(__ice_render_source_location);
let __ice_rendered: __IceElement<'_, __DucktapeMessage> = { let __a11y_key = format!("{}/@container:1064", __ice_component_0526573746f726553637265656e_scope_3975); let __container_content: __IceElement<'_, __DucktapeMessage> = {
#[cfg(test)]
let __ice_render_source_location = ::ui_lang_runtime::testing::Location::new("components/onboarding.ice", 1073, 1, "rendered view node");
#[cfg(test)]
let __ice_render_source = ::ui_lang_runtime::testing::push_render_source(__ice_render_source_location);
let __ice_rendered: __IceElement<'_, __DucktapeMessage> = (|| { let __slot_content: __IceElement<'_, __DucktapeMessage> = {
#[cfg(test)]
let __ice_render_source_location = ::ui_lang_runtime::testing::Location::new("components/onboarding.ice", 133, 1, "rendered view node");
#[cfg(test)]
let __ice_render_source = ::ui_lang_runtime::testing::push_render_source(__ice_render_source_location);
let __ice_rendered: __IceElement<'_, __DucktapeMessage> = (|| { let __slot_content: __IceElement<'_, __DucktapeMessage> = {
#[cfg(test)]
let __ice_render_source_location = ::ui_lang_runtime::testing::Location::new("view.ice", 59, 1, "rendered view node");
#[cfg(test)]
let __ice_render_source = ::ui_lang_runtime::testing::push_render_source(__ice_render_source_location);
let __ice_rendered: __IceElement<'_, __DucktapeMessage> = { let __ice_node_scope = format!("{}/restore-words", __ice_node_scope); { let __a11y_key = __ice_node_scope.clone(); let __a11y_id = ::ui_lang_runtime::StableId::new(&__a11y_key); let __disabled = (*self.__ice_derived_mutation_busy()); let __secure = true; let __role = if __secure { ::ui_lang_runtime::Role::PasswordInput } else { ::ui_lang_runtime::Role::TextInput }; let __input = ::ui_lang_runtime::accessible(::iced::widget::text_input("24 words, space-separated", &self.__ice_secrets.text("restore_words")).id(::iced::widget::Id::from(__a11y_key.clone())).padding(::iced::Padding { top: 11.0, right: 13.0, bottom: 11.0, left: 13.0 }).width(::iced::Fill).secure(__secure).width(::iced::Fill).padding(0.0 as f32).size(((12.0) as f32).max(f32::EPSILON).min(f32::MAX)).line_height(::iced::widget::text::LineHeight::Relative(((1.2) as f32).max(f32::EPSILON).min(f32::MAX))).font(::iced::Font { family: ::iced::font::Family::Name("Geist Mono"), weight: ::iced::font::Weight::Normal, stretch: ::iced::font::Stretch::Normal, style: ::iced::font::Style::Normal }).on_input_maybe(if __disabled { None } else { Some(move |__text| __DucktapeMessage::__SecretTyped(::std::string::String::from("restore_words"), __text)) }).style(move |__theme, __status| { let mut __style = ::iced::widget::text_input::default(__theme, __status); __style.background = __ice_palette.colors[3].into(); __style.border.color = __ice_palette.colors[39]; __style.border.width = 1.0; __style.border.radius = 10.0.into(); __style.background = ::iced::Background::Color(::iced::Color::from_rgba8(0, 0, 0, 0.000000)); __style.border.color = ::iced::Color::from_rgba8(0, 0, 0, 0.000000); __style.border.width = 0.0 as f32; __style.border.radius = ::iced::border::Radius { top_left: ((0.0) as f32).max(0.0).min(f32::MAX), top_right: ((0.0) as f32).max(0.0).min(f32::MAX), bottom_right: ((0.0) as f32).max(0.0).min(f32::MAX), bottom_left: ((0.0) as f32).max(0.0).min(f32::MAX) }; __style.placeholder = __ice_palette.colors[73]; __style.value = __ice_palette.colors[4]; __style.selection = { let mut __color = __ice_palette.colors[4]; __color.a = 0.180000; __color }; if matches!(__status, ::iced::widget::text_input::Status::Focused { .. }) { __style.border.color = __ice_palette.colors[42]; } match __status { ::iced::widget::text_input::Status::Disabled => { __style.value = __ice_palette.colors[72]; } _ => {} } __style }), __a11y_id, __role).logical_id_maybe(::core::cfg!(test).then_some(&*__a11y_key)).focus_id(::iced::widget::Id::from(__a11y_key)).label("Recovery phrase".to_owned()).value_maybe(::std::option::Option::None).disabled(__disabled); __input.into() } };
#[cfg(test)]
drop(__ice_render_source);
__ice_rendered
}; __slot_content })();
#[cfg(test)]
drop(__ice_render_source);
__ice_rendered
}; __slot_content })();
#[cfg(test)]
drop(__ice_render_source);
__ice_rendered
}; let __container = {  ::iced::widget::container(__container_content).padding(::ui_lang_runtime::bounded_padding(12.0, 14.0, 12.0, 14.0)).width(::iced::Fill).style(move |__theme| { let mut __style = ::iced::widget::container::Style::default(); __style.background = ::std::option::Option::Some(::iced::Background::Color(__ice_palette.colors[3])); __style.border.color = __ice_palette.colors[7]; __style.border.width = 1.5 as f32; __style.border.radius = ::iced::border::Radius { top_left: ((10.0) as f32).max(0.0).min(f32::MAX), top_right: ((10.0) as f32).max(0.0).min(f32::MAX), bottom_right: ((10.0) as f32).max(0.0).min(f32::MAX), bottom_left: ((10.0) as f32).max(0.0).min(f32::MAX) }; __style }) }; ::ui_lang_runtime::accessible(__container, ::ui_lang_runtime::StableId::new(&__a11y_key), ::ui_lang_runtime::Role::GenericContainer).logical_id_maybe(::core::cfg!(test).then_some(__a11y_key)).into() };
#[cfg(test)]
drop(__ice_render_source);
__ice_rendered
}; let __container = {  ::iced::widget::container(__container_content).padding(::ui_lang_runtime::bounded_padding(8.0, 0.0, 0.0, 0.0)).width(::iced::Fill) }; ::ui_lang_runtime::accessible(__container, ::ui_lang_runtime::StableId::new(&__a11y_key), ::ui_lang_runtime::Role::GenericContainer).logical_id_maybe(::core::cfg!(test).then_some(__a11y_key)).into() };
#[cfg(test)]
drop(__ice_render_source);
__ice_rendered
}); __children.push({
#[cfg(test)]
let __ice_render_source_location = ::ui_lang_runtime::testing::Location::new("components/onboarding.ice", 1074, 1, "rendered view node");
#[cfg(test)]
let __ice_render_source = ::ui_lang_runtime::testing::push_render_source(__ice_render_source_location);
let __ice_rendered: __IceElement<'_, __DucktapeMessage> = { let __a11y_key = format!("{}/@container:1074", __ice_component_0526573746f726553637265656e_scope_3975); let __container_content: __IceElement<'_, __DucktapeMessage> = {
#[cfg(test)]
let __ice_render_source_location = ::ui_lang_runtime::testing::Location::new("components/onboarding.ice", 1075, 1, "rendered view node");
#[cfg(test)]
let __ice_render_source = ::ui_lang_runtime::testing::push_render_source(__ice_render_source_location);
let __ice_rendered: __IceElement<'_, __DucktapeMessage> = { let __a11y_key = format!("{}/@text:1075", __ice_component_0526573746f726553637265656e_scope_3975); let __text_value = ("NEW PASSWORD").to_string(); let __text = ::iced::widget::text(__text_value.clone()).size(((10.0) as f32).max(f32::EPSILON).min(f32::MAX)).wrapping(::iced::widget::text::Wrapping::None).font(::iced::Font { family: ::iced::font::Family::Name("Geist Mono"), weight: ::iced::font::Weight::Semibold, stretch: ::iced::font::Stretch::Normal, style: ::iced::font::Style::Normal }).color(__ice_palette.colors[73]); let __text = ::ui_lang_runtime::selectable_text(__text); ::ui_lang_runtime::accessible(__text, ::ui_lang_runtime::StableId::new(&__a11y_key), ::ui_lang_runtime::Role::Label).logical_id_maybe(::core::cfg!(test).then_some(__a11y_key)).value(__text_value).into() };
#[cfg(test)]
drop(__ice_render_source);
__ice_rendered
}; let __container = {  ::iced::widget::container(__container_content).padding(::ui_lang_runtime::bounded_padding(14.0, 0.0, 0.0, 0.0)).width(::iced::Fill) }; ::ui_lang_runtime::accessible(__container, ::ui_lang_runtime::StableId::new(&__a11y_key), ::ui_lang_runtime::Role::GenericContainer).logical_id_maybe(::core::cfg!(test).then_some(__a11y_key)).into() };
#[cfg(test)]
drop(__ice_render_source);
__ice_rendered
}); __children.push({
#[cfg(test)]
let __ice_render_source_location = ::ui_lang_runtime::testing::Location::new("components/onboarding.ice", 1081, 1, "rendered view node");
#[cfg(test)]
let __ice_render_source = ::ui_lang_runtime::testing::push_render_source(__ice_render_source_location);
let __ice_rendered: __IceElement<'_, __DucktapeMessage> = { let __a11y_key = format!("{}/@container:1081", __ice_component_0526573746f726553637265656e_scope_3975); let __container_content: __IceElement<'_, __DucktapeMessage> = {
#[cfg(test)]
let __ice_render_source_location = ::ui_lang_runtime::testing::Location::new("components/onboarding.ice", 1082, 1, "rendered view node");
#[cfg(test)]
let __ice_render_source = ::ui_lang_runtime::testing::push_render_source(__ice_render_source_location);
let __ice_rendered: __IceElement<'_, __DucktapeMessage> = { let __a11y_key = format!("{}/@container:1082", __ice_component_0526573746f726553637265656e_scope_3975); let __container_content: __IceElement<'_, __DucktapeMessage> = {
#[cfg(test)]
let __ice_render_source_location = ::ui_lang_runtime::testing::Location::new("components/onboarding.ice", 1091, 1, "rendered view node");
#[cfg(test)]
let __ice_render_source = ::ui_lang_runtime::testing::push_render_source(__ice_render_source_location);
let __ice_rendered: __IceElement<'_, __DucktapeMessage> = { let __ice_node_scope = format!("{}/restore-password", __ice_node_scope); { let __a11y_key = __ice_node_scope.clone(); let __a11y_id = ::ui_lang_runtime::StableId::new(&__a11y_key); let __disabled = (*self.__ice_derived_hub_busy()); let __secure = true; let __role = if __secure { ::ui_lang_runtime::Role::PasswordInput } else { ::ui_lang_runtime::Role::TextInput }; let __input = ::ui_lang_runtime::accessible(::iced::widget::text_input("at least 8 characters", &self.__ice_component_0526573746f726553637265656e.get(&__ice_component_0526573746f726553637265656e_scope_3975).map_or_else(|| "".to_owned(), |__state| __state.pw.clone())).id(::iced::widget::Id::from(__a11y_key.clone())).padding(::iced::Padding { top: 11.0, right: 13.0, bottom: 11.0, left: 13.0 }).width(::iced::Fill).secure(__secure).width(::iced::Fill).padding(0.0 as f32).size(((13.0) as f32).max(f32::EPSILON).min(f32::MAX)).line_height(::iced::widget::text::LineHeight::Relative(((1.2) as f32).max(f32::EPSILON).min(f32::MAX))).font(::iced::Font { family: ::iced::font::Family::Name("Geist Mono"), weight: ::iced::font::Weight::Normal, stretch: ::iced::font::Stretch::Normal, style: ::iced::font::Style::Normal }).on_input_maybe(if __disabled { None } else { Some({ let __scope = (__ice_component_0526573746f726553637265656e_scope_3975).clone(); move |__value| __DucktapeMessage::__0C526573746f726553637265656eB7077(__scope.clone(), __value) }) }).on_submit_maybe(if __disabled { None } else { Some((__ice_cb_18)(self.__ice_component_0526573746f726553637265656e.get(&__ice_component_0526573746f726553637265656e_scope_3975).map_or_else(|| "restored".to_owned(), |__state| __state.name_draft.clone()), self.__ice_component_0526573746f726553637265656e.get(&__ice_component_0526573746f726553637265656e_scope_3975).map_or_else(|| "".to_owned(), |__state| __state.pw.clone()))) }).style(move |__theme, __status| { let mut __style = ::iced::widget::text_input::default(__theme, __status); __style.background = __ice_palette.colors[3].into(); __style.border.color = __ice_palette.colors[39]; __style.border.width = 1.0; __style.border.radius = 10.0.into(); __style.background = ::iced::Background::Color(::iced::Color::from_rgba8(0, 0, 0, 0.000000)); __style.border.color = ::iced::Color::from_rgba8(0, 0, 0, 0.000000); __style.border.width = 0.0 as f32; __style.border.radius = ::iced::border::Radius { top_left: ((0.0) as f32).max(0.0).min(f32::MAX), top_right: ((0.0) as f32).max(0.0).min(f32::MAX), bottom_right: ((0.0) as f32).max(0.0).min(f32::MAX), bottom_left: ((0.0) as f32).max(0.0).min(f32::MAX) }; __style.placeholder = __ice_palette.colors[73]; __style.value = __ice_palette.colors[4]; __style.selection = { let mut __color = __ice_palette.colors[4]; __color.a = 0.180000; __color }; if matches!(__status, ::iced::widget::text_input::Status::Focused { .. }) { __style.border.color = __ice_palette.colors[42]; } match __status { ::iced::widget::text_input::Status::Disabled => { __style.value = __ice_palette.colors[72]; } _ => {} } __style }), __a11y_id, __role).logical_id_maybe(::core::cfg!(test).then_some(&*__a11y_key)).focus_id(::iced::widget::Id::from(__a11y_key)).label("New password".to_owned()).value_maybe((!__secure).then(|| (self.__ice_component_0526573746f726553637265656e.get(&__ice_component_0526573746f726553637265656e_scope_3975).map_or_else(|| "".to_owned(), |__state| __state.pw.clone())).to_owned())).disabled(__disabled); __input.into() } };
#[cfg(test)]
drop(__ice_render_source);
__ice_rendered
}; let __container = {  ::iced::widget::container(__container_content).padding(::ui_lang_runtime::bounded_padding(12.0, 14.0, 12.0, 14.0)).width(::iced::Fill).style(move |__theme| { let mut __style = ::iced::widget::container::Style::default(); __style.background = ::std::option::Option::Some(::iced::Background::Color(__ice_palette.colors[3])); __style.border.color = __ice_palette.colors[7]; __style.border.width = 1.5 as f32; __style.border.radius = ::iced::border::Radius { top_left: ((10.0) as f32).max(0.0).min(f32::MAX), top_right: ((10.0) as f32).max(0.0).min(f32::MAX), bottom_right: ((10.0) as f32).max(0.0).min(f32::MAX), bottom_left: ((10.0) as f32).max(0.0).min(f32::MAX) }; __style }) }; ::ui_lang_runtime::accessible(__container, ::ui_lang_runtime::StableId::new(&__a11y_key), ::ui_lang_runtime::Role::GenericContainer).logical_id_maybe(::core::cfg!(test).then_some(__a11y_key)).into() };
#[cfg(test)]
drop(__ice_render_source);
__ice_rendered
}; let __container = {  ::iced::widget::container(__container_content).padding(::ui_lang_runtime::bounded_padding(8.0, 0.0, 0.0, 0.0)).width(::iced::Fill) }; ::ui_lang_runtime::accessible(__container, ::ui_lang_runtime::StableId::new(&__a11y_key), ::ui_lang_runtime::Role::GenericContainer).logical_id_maybe(::core::cfg!(test).then_some(__a11y_key)).into() };
#[cfg(test)]
drop(__ice_render_source);
__ice_rendered
}); __children.push({
#[cfg(test)]
let __ice_render_source_location = ::ui_lang_runtime::testing::Location::new("components/onboarding.ice", 1106, 1, "rendered view node");
#[cfg(test)]
let __ice_render_source = ::ui_lang_runtime::testing::push_render_source(__ice_render_source_location);
let __ice_rendered: __IceElement<'_, __DucktapeMessage> = { let __a11y_key = format!("{}/@container:1106", __ice_component_0526573746f726553637265656e_scope_3975); let __container_content: __IceElement<'_, __DucktapeMessage> = {
#[cfg(test)]
let __ice_render_source_location = ::ui_lang_runtime::testing::Location::new("components/onboarding.ice", 1107, 1, "rendered view node");
#[cfg(test)]
let __ice_render_source = ::ui_lang_runtime::testing::push_render_source(__ice_render_source_location);
let __ice_rendered: __IceElement<'_, __DucktapeMessage> = { let __a11y_key = format!("{}/@button:1107", __ice_component_0526573746f726553637265656e_scope_3975); let __a11y_id = ::ui_lang_runtime::StableId::new(&__a11y_key); let __disabled = ((((*self.__ice_derived_hub_busy()) || (self.__ice_secrets.text("restore_words")).is_empty()) || ((*self.__ice_component_0526573746f726553637265656e.get(&__ice_component_0526573746f726553637265656e_scope_3975).map_or(&self.__ice_component_0526573746f726553637265656e_initial.pw, |__ice_local| &__ice_local.pw))).is_empty()) || ((*self.__ice_component_0526573746f726553637265656e.get(&__ice_component_0526573746f726553637265656e_scope_3975).map_or(&self.__ice_component_0526573746f726553637265656e_initial.name_draft, |__ice_local| &__ice_local.name_draft))).is_empty()); let __activate = (__ice_cb_18)(self.__ice_component_0526573746f726553637265656e.get(&__ice_component_0526573746f726553637265656e_scope_3975).map_or_else(|| "restored".to_owned(), |__state| __state.name_draft.clone()), self.__ice_component_0526573746f726553637265656e.get(&__ice_component_0526573746f726553637265656e_scope_3975).map_or_else(|| "".to_owned(), |__state| __state.pw.clone())); let __button_content: __IceElement<'_, __DucktapeMessage> = {
#[cfg(test)]
let __ice_render_source_location = ::ui_lang_runtime::testing::Location::new("components/onboarding.ice", 1116, 1, "rendered view node");
#[cfg(test)]
let __ice_render_source = ::ui_lang_runtime::testing::push_render_source(__ice_render_source_location);
let __ice_rendered: __IceElement<'_, __DucktapeMessage> = { let __a11y_key = format!("{}/@text:1116", __ice_component_0526573746f726553637265656e_scope_3975); let __text_value = ("Restore →").to_string(); let __text = ::iced::widget::text(__text_value.clone()).size(((13.5) as f32).max(f32::EPSILON).min(f32::MAX)).width(::iced::Fill).align_x(::iced::widget::text::Alignment::Center).wrapping(::iced::widget::text::Wrapping::None).font(::iced::Font { family: ::iced::font::Family::Name("Geist"), weight: ::iced::font::Weight::Semibold, stretch: ::iced::font::Stretch::Normal, style: ::iced::font::Style::Normal }).color(__ice_palette.colors[9]); let __text = ::ui_lang_runtime::selectable_text(__text); ::ui_lang_runtime::accessible(__text, ::ui_lang_runtime::StableId::new(&__a11y_key), ::ui_lang_runtime::Role::Label).logical_id_maybe(::core::cfg!(test).then_some(__a11y_key)).value(__text_value).into() };
#[cfg(test)]
drop(__ice_render_source);
__ice_rendered
}; let __button = ::iced::widget::button(__button_content).padding(::iced::Padding { top: 13.0, right: 0.0, bottom: 13.0, left: 0.0 }).width(::iced::Fill).on_press_maybe(if __disabled { None } else { Some(__activate.clone()) }).style(move |__theme, __status| { let mut __style = ::iced::widget::button::primary(__theme, __status); let __background: Option<::iced::Color> = match __status { ::iced::widget::button::Status::Hovered => Some(__ice_palette.colors[8]), ::iced::widget::button::Status::Pressed => Some({ let mut __color = __ice_palette.colors[7]; __color.a = 0.800000; __color }), ::iced::widget::button::Status::Disabled => Some(__ice_palette.colors[7]), _ => Some(__ice_palette.colors[7]) }; if let Some(__background) = __background { __style.background = Some(::iced::Background::Color(__background)); } __style.text_color = __ice_palette.colors[9]; __style.border.radius = 10.0.into(); if matches!(__status, ::iced::widget::button::Status::Disabled) { __style.background = Some(__ice_palette.colors[10].into()); __style.text_color = __ice_palette.colors[11]; } __style }); ::ui_lang_runtime::accessible(__button, __a11y_id, ::ui_lang_runtime::Role::Button).logical_id_maybe(::core::cfg!(test).then_some(&*__a11y_key)).focus_id(::iced::widget::Id::from(__a11y_key)).label("Restore".to_owned()).disabled(__disabled).on_activate_maybe(if __disabled { None } else { Some(__activate) }).focus_ring(__ice_palette.colors[42], 10.0).into() };
#[cfg(test)]
drop(__ice_render_source);
__ice_rendered
}; let __container = {  ::iced::widget::container(__container_content).padding(::ui_lang_runtime::bounded_padding(18.0, 0.0, 0.0, 0.0)).width(::iced::Fill) }; ::ui_lang_runtime::accessible(__container, ::ui_lang_runtime::StableId::new(&__a11y_key), ::ui_lang_runtime::Role::GenericContainer).logical_id_maybe(::core::cfg!(test).then_some(__a11y_key)).into() };
#[cfg(test)]
drop(__ice_render_source);
__ice_rendered
}); __children.push({
#[cfg(test)]
let __ice_render_source_location = ::ui_lang_runtime::testing::Location::new("components/onboarding.ice", 1124, 1, "rendered view node");
#[cfg(test)]
let __ice_render_source = ::ui_lang_runtime::testing::push_render_source(__ice_render_source_location);
let __ice_rendered: __IceElement<'_, __DucktapeMessage> = { let __ice_component_04f6e626f617264696e674572726f72_scope_4975 = format!("{}/OnboardingError@4975", __ice_component_0526573746f726553637265656e_scope_3975); ::ui_lang_runtime::rev_memo(695u64, (self.__ice_rev[131], __ice_palette.name), ::ui_lang_runtime::grow_stack(|| self.__ice_component_use_5(__ice_palette, __ice_component_04f6e626f617264696e674572726f72_scope_4975))).into() };
#[cfg(test)]
drop(__ice_render_source);
__ice_rendered
}); let __child_count = __children.len(); let __children = __children.into_iter().map(|__child| ::ui_lang_runtime::bounded_fill_element(__child, __child_count, false)).collect::<::std::vec::Vec<_>>(); let __layout = ::iced::widget::column(__children).spacing(::ui_lang_runtime::bounded_spacing(0.0, __child_count)).width(428.0 as f32); let __content = ::iced::widget::container(__layout); let __layout_content: __IceElement<'_, __DucktapeMessage> = __content.into(); let __a11y_key = __ice_node_scope.as_str(); ::ui_lang_runtime::accessible(__layout_content, ::ui_lang_runtime::StableId::new(&__a11y_key), ::ui_lang_runtime::Role::GenericContainer).logical_id_maybe(::core::cfg!(test).then_some(__a11y_key)).into() } };
#[cfg(test)]
drop(__ice_render_source);
__ice_rendered
}; __component_content };
#[cfg(test)]
drop(__ice_render_source);
__ice_rendered
}); }, HubStep::Networks => { __children.push({
#[cfg(test)]
let __ice_render_source_location = ::ui_lang_runtime::testing::Location::new("components/onboarding.ice", 135, 1, "rendered view node");
#[cfg(test)]
let __ice_render_source = ::ui_lang_runtime::testing::push_render_source(__ice_render_source_location);
let __ice_rendered: __IceElement<'_, __DucktapeMessage> = { let __ice_node_scope = format!("{}/networks", __ice_node_scope); { let __ice_component_04e6574776f726b7353637265656e_scope_3986 = __ice_node_scope.clone(); ::ui_lang_runtime::rev_memo(507u64, (self.__ice_component_04e6574776f726b7353637265656e.get(&__ice_component_04e6574776f726b7353637265656e_scope_3986).map_or(0, |__state| __state.__ice_rev[0]), self.__ice_rev[126], self.__ice_rev[127], self.__ice_rev[131], self.__ice_rev[132], self.__ice_rev[19], __ice_palette.name), ::ui_lang_runtime::grow_stack(|| self.__ice_component_use_15(__ice_palette, __ice_component_04e6574776f726b7353637265656e_scope_3986, (move |__event_0| __DucktapeMessage::ConnectRemoteSubmit(__event_0)).clone(), (move |__event_0| __DucktapeMessage::ForgetNetworkSubmit(__event_0)).clone(), (move || __DucktapeMessage::GoJoin).clone(), (move || __DucktapeMessage::OpenNetworkSubmit).clone(), (move |__event_0| __DucktapeMessage::PickNetwork(__event_0)).clone()))).into() } };
#[cfg(test)]
drop(__ice_render_source);
__ice_rendered
}); }, HubStep::Provisioning => { __children.push({
#[cfg(test)]
let __ice_render_source_location = ::ui_lang_runtime::testing::Location::new("components/onboarding.ice", 148, 1, "rendered view node");
#[cfg(test)]
let __ice_render_source = ::ui_lang_runtime::testing::push_render_source(__ice_render_source_location);
let __ice_rendered: __IceElement<'_, __DucktapeMessage> = { let __ice_component_050726f766973696f6e696e6753637265656e_scope_3999 = format!("{}/ProvisioningScreen@3999", __ice_component_0487562436f6c756d6e_scope_10219); ::ui_lang_runtime::rev_memo(508u64, (self.__ice_rev[130], self.__ice_rev[131], self.__ice_rev[134], self.__ice_rev[135], __ice_palette.name), ::ui_lang_runtime::grow_stack(|| self.__ice_component_use_31(__ice_palette, __ice_component_050726f766973696f6e696e6753637265656e_scope_3999))).into() };
#[cfg(test)]
drop(__ice_render_source);
__ice_rendered
}); }, HubStep::Live => { __children.push({
#[cfg(test)]
let __ice_render_source_location = ::ui_lang_runtime::testing::Location::new("components/onboarding.ice", 155, 1, "rendered view node");
#[cfg(test)]
let __ice_render_source = ::ui_lang_runtime::testing::push_render_source(__ice_render_source_location);
let __ice_rendered: __IceElement<'_, __DucktapeMessage> = { let __ice_component_04c69766553637265656e_scope_4006 = format!("{}/LiveScreen@4006", __ice_component_0487562436f6c756d6e_scope_10219); ::ui_lang_runtime::rev_memo(509u64, (self.__ice_rev[130], self.__ice_rev[131], self.__ice_rev[132], self.__ice_rev[133], self.__ice_rev[14], self.__ice_rev[19], self.__ice_rev[57], __ice_palette.name), ::ui_lang_runtime::grow_stack(|| self.__ice_component_use_35(__ice_palette, __ice_component_04c69766553637265656e_scope_4006, (move || __DucktapeMessage::CopyOnboardingInvite).clone(), (move || __DucktapeMessage::EnterConsole).clone(), (move || __DucktapeMessage::GoNetworks).clone()))).into() };
#[cfg(test)]
drop(__ice_render_source);
__ice_rendered
}); }, HubStep::Join => { __children.push({
#[cfg(test)]
let __ice_render_source_location = ::ui_lang_runtime::testing::Location::new("components/onboarding.ice", 170, 1, "rendered view node");
#[cfg(test)]
let __ice_render_source = ::ui_lang_runtime::testing::push_render_source(__ice_render_source_location);
let __ice_rendered: __IceElement<'_, __DucktapeMessage> = { let __ice_component_04a6f696e53637265656e_scope_4021 = format!("{}/JoinScreen@4021", __ice_component_0487562436f6c756d6e_scope_10219); let __ice_cb_0 = (move || __DucktapeMessage::CloseLaunchWindow).clone(); let __ice_cb_1 = (move |__event_0| __DucktapeMessage::ConfirmPhraseSubmit(__event_0)).clone(); let __ice_cb_2 = (move |__event_0| __DucktapeMessage::ConnectRemoteSubmit(__event_0)).clone(); let __ice_cb_3 = (move || __DucktapeMessage::CopyOnboardingInvite).clone(); let __ice_cb_4 = (move || __DucktapeMessage::DragLaunchWindow).clone(); let __ice_cb_5 = (move || __DucktapeMessage::EnterConsole).clone(); let __ice_cb_6 = (move |__event_0| __DucktapeMessage::ForgetNetworkSubmit(__event_0)).clone(); let __ice_cb_7 = (move || __DucktapeMessage::GoJoin).clone(); let __ice_cb_8 = (move || __DucktapeMessage::GoLogin).clone(); let __ice_cb_9 = (move || __DucktapeMessage::GoNetworks).clone(); let __ice_cb_10 = (move || __DucktapeMessage::GoRestore).clone(); let __ice_cb_11 = (move || __DucktapeMessage::JoinNetworkSubmit).clone(); let __ice_cb_12 = (move || __DucktapeMessage::LoginSkip).clone(); let __ice_cb_13 = (move || __DucktapeMessage::OpenNetworkSubmit).clone(); let __ice_cb_14 = (move |__event_0| __DucktapeMessage::PasswordSubmit(__event_0)).clone(); let __ice_cb_15 = (move || __DucktapeMessage::PhraseWrittenDown).clone(); let __ice_cb_16 = (move |__event_0| __DucktapeMessage::PickNetwork(__event_0)).clone(); let __ice_cb_17 = (move |__event_0| __DucktapeMessage::PickWallet(__event_0)).clone(); let __ice_cb_18 = (move |__event_0, __event_1| __DucktapeMessage::RestoreSubmit(__event_0, __event_1)).clone(); let __ice_cb_19 = (move || __DucktapeMessage::ShowPhraseAgain).clone(); let __ice_cb_20 = (move |__event_0| __DucktapeMessage::UnlockSubmit(__event_0)).clone(); let __ice_cb_21 = (move || __DucktapeMessage::WelcomeCancel).clone(); let __ice_cb_22 = (move |__event_0| __DucktapeMessage::WelcomeCreateSubmit(__event_0)).clone(); let __ice_cb_23 = (move || __DucktapeMessage::WelcomeDesktop).clone(); let __ice_cb_24 = (move || __DucktapeMessage::WelcomeLoginSubmit).clone(); let __ice_cb_25 = (move || __DucktapeMessage::WelcomeSkip).clone(); let __component_content: __IceElement<'_, __DucktapeMessage> = {
#[cfg(test)]
let __ice_render_source_location = ::ui_lang_runtime::testing::Location::new("components/onboarding.ice", 1907, 1, "rendered view node");
#[cfg(test)]
let __ice_render_source = ::ui_lang_runtime::testing::push_render_source(__ice_render_source_location);
let __ice_rendered: __IceElement<'_, __DucktapeMessage> = { let __ice_node_scope = format!("{}/root", __ice_component_04a6f696e53637265656e_scope_4021); { let mut __children: ::std::vec::Vec<__IceElement<'_, __DucktapeMessage>> = ::std::vec::Vec::new(); __children.push({
#[cfg(test)]
let __ice_render_source_location = ::ui_lang_runtime::testing::Location::new("components/onboarding.ice", 1908, 1, "rendered view node");
#[cfg(test)]
let __ice_render_source = ::ui_lang_runtime::testing::push_render_source(__ice_render_source_location);
let __ice_rendered: __IceElement<'_, __DucktapeMessage> = { let __ice_component_04261636b546f4e6574776f726b73_scope_5759 = format!("{}/BackToNetworks@5759", __ice_component_04a6f696e53637265656e_scope_4021); ::ui_lang_runtime::rev_memo(882u64, (__ice_palette.name), ::ui_lang_runtime::grow_stack(|| self.__ice_component_use_0(__ice_palette, __ice_component_04261636b546f4e6574776f726b73_scope_5759, (__ice_cb_9).clone()))).into() };
#[cfg(test)]
drop(__ice_render_source);
__ice_rendered
}); __children.push({
#[cfg(test)]
let __ice_render_source_location = ::ui_lang_runtime::testing::Location::new("components/onboarding.ice", 1911, 1, "rendered view node");
#[cfg(test)]
let __ice_render_source = ::ui_lang_runtime::testing::push_render_source(__ice_render_source_location);
let __ice_rendered: __IceElement<'_, __DucktapeMessage> = { let __a11y_key = format!("{}/@container:1911", __ice_component_04a6f696e53637265656e_scope_4021); let __container_content: __IceElement<'_, __DucktapeMessage> = {
#[cfg(test)]
let __ice_render_source_location = ::ui_lang_runtime::testing::Location::new("components/onboarding.ice", 1912, 1, "rendered view node");
#[cfg(test)]
let __ice_render_source = ::ui_lang_runtime::testing::push_render_source(__ice_render_source_location);
let __ice_rendered: __IceElement<'_, __DucktapeMessage> = { let __a11y_key = format!("{}/@text:1912", __ice_component_04a6f696e53637265656e_scope_4021); let __text_value = ("Join a network").to_string(); let __text = ::iced::widget::text(__text_value.clone()).size(((20.0) as f32).max(f32::EPSILON).min(f32::MAX)).width(::iced::Fill).wrapping(::iced::widget::text::Wrapping::None).font(::iced::Font { family: ::iced::font::Family::Name("Geist"), weight: ::iced::font::Weight::Semibold, stretch: ::iced::font::Stretch::Normal, style: ::iced::font::Style::Normal }).color(__ice_palette.colors[7]); let __text = ::ui_lang_runtime::selectable_text(__text); ::ui_lang_runtime::accessible(__text, ::ui_lang_runtime::StableId::new(&__a11y_key), ::ui_lang_runtime::Role::Label).logical_id_maybe(::core::cfg!(test).then_some(__a11y_key)).value(__text_value).into() };
#[cfg(test)]
drop(__ice_render_source);
__ice_rendered
}; let __container = {  ::iced::widget::container(__container_content).padding(::ui_lang_runtime::bounded_padding(16.0, 0.0, 0.0, 0.0)).width(::iced::Fill) }; ::ui_lang_runtime::accessible(__container, ::ui_lang_runtime::StableId::new(&__a11y_key), ::ui_lang_runtime::Role::GenericContainer).logical_id_maybe(::core::cfg!(test).then_some(__a11y_key)).into() };
#[cfg(test)]
drop(__ice_render_source);
__ice_rendered
}); __children.push({
#[cfg(test)]
let __ice_render_source_location = ::ui_lang_runtime::testing::Location::new("components/onboarding.ice", 1919, 1, "rendered view node");
#[cfg(test)]
let __ice_render_source = ::ui_lang_runtime::testing::push_render_source(__ice_render_source_location);
let __ice_rendered: __IceElement<'_, __DucktapeMessage> = { let __a11y_key = format!("{}/@container:1919", __ice_component_04a6f696e53637265656e_scope_4021); let __container_content: __IceElement<'_, __DucktapeMessage> = {
#[cfg(test)]
let __ice_render_source_location = ::ui_lang_runtime::testing::Location::new("components/onboarding.ice", 1920, 1, "rendered view node");
#[cfg(test)]
let __ice_render_source = ::ui_lang_runtime::testing::push_render_source(__ice_render_source_location);
let __ice_rendered: __IceElement<'_, __DucktapeMessage> = { let __a11y_key = format!("{}/@text:1920", __ice_component_04a6f696e53637265656e_scope_4021); let __text_value = ("Paste an invite to materialize this device's node, download the finalized history, verify it, and ask to join.").to_string(); let __text = ::iced::widget::text(__text_value.clone()).size(((13.0) as f32).max(f32::EPSILON).min(f32::MAX)).width(::iced::Fill).line_height(::iced::widget::text::LineHeight::Relative(((1.5) as f32).max(f32::EPSILON).min(f32::MAX))).color(__ice_palette.colors[70]); let __text = ::ui_lang_runtime::selectable_text(__text); ::ui_lang_runtime::accessible(__text, ::ui_lang_runtime::StableId::new(&__a11y_key), ::ui_lang_runtime::Role::Label).logical_id_maybe(::core::cfg!(test).then_some(__a11y_key)).value(__text_value).into() };
#[cfg(test)]
drop(__ice_render_source);
__ice_rendered
}; let __container = {  ::iced::widget::container(__container_content).padding(::ui_lang_runtime::bounded_padding(5.0, 0.0, 0.0, 0.0)).width(::iced::Fill) }; ::ui_lang_runtime::accessible(__container, ::ui_lang_runtime::StableId::new(&__a11y_key), ::ui_lang_runtime::Role::GenericContainer).logical_id_maybe(::core::cfg!(test).then_some(__a11y_key)).into() };
#[cfg(test)]
drop(__ice_render_source);
__ice_rendered
}); __children.push({
#[cfg(test)]
let __ice_render_source_location = ::ui_lang_runtime::testing::Location::new("components/onboarding.ice", 1926, 1, "rendered view node");
#[cfg(test)]
let __ice_render_source = ::ui_lang_runtime::testing::push_render_source(__ice_render_source_location);
let __ice_rendered: __IceElement<'_, __DucktapeMessage> = { let __a11y_key = format!("{}/@container:1926", __ice_component_04a6f696e53637265656e_scope_4021); let __container_content: __IceElement<'_, __DucktapeMessage> = {
#[cfg(test)]
let __ice_render_source_location = ::ui_lang_runtime::testing::Location::new("components/onboarding.ice", 1931, 1, "rendered view node");
#[cfg(test)]
let __ice_render_source = ::ui_lang_runtime::testing::push_render_source(__ice_render_source_location);
let __ice_rendered: __IceElement<'_, __DucktapeMessage> = { let __a11y_key = format!("{}/@text:1931", __ice_component_04a6f696e53637265656e_scope_4021); let __text_value = ("INVITE").to_string(); let __text = ::iced::widget::text(__text_value.clone()).size(((10.0) as f32).max(f32::EPSILON).min(f32::MAX)).wrapping(::iced::widget::text::Wrapping::None).font(::iced::Font { family: ::iced::font::Family::Name("Geist Mono"), weight: ::iced::font::Weight::Semibold, stretch: ::iced::font::Stretch::Normal, style: ::iced::font::Style::Normal }).color(__ice_palette.colors[73]); let __text = ::ui_lang_runtime::selectable_text(__text); ::ui_lang_runtime::accessible(__text, ::ui_lang_runtime::StableId::new(&__a11y_key), ::ui_lang_runtime::Role::Label).logical_id_maybe(::core::cfg!(test).then_some(__a11y_key)).value(__text_value).into() };
#[cfg(test)]
drop(__ice_render_source);
__ice_rendered
}; let __container = {  ::iced::widget::container(__container_content).padding(::ui_lang_runtime::bounded_padding(20.0, 0.0, 0.0, 0.0)).width(::iced::Fill) }; ::ui_lang_runtime::accessible(__container, ::ui_lang_runtime::StableId::new(&__a11y_key), ::ui_lang_runtime::Role::GenericContainer).logical_id_maybe(::core::cfg!(test).then_some(__a11y_key)).into() };
#[cfg(test)]
drop(__ice_render_source);
__ice_rendered
}); __children.push({
#[cfg(test)]
let __ice_render_source_location = ::ui_lang_runtime::testing::Location::new("components/onboarding.ice", 1937, 1, "rendered view node");
#[cfg(test)]
let __ice_render_source = ::ui_lang_runtime::testing::push_render_source(__ice_render_source_location);
let __ice_rendered: __IceElement<'_, __DucktapeMessage> = { let __a11y_key = format!("{}/@container:1937", __ice_component_04a6f696e53637265656e_scope_4021); let __container_content: __IceElement<'_, __DucktapeMessage> = {
#[cfg(test)]
let __ice_render_source_location = ::ui_lang_runtime::testing::Location::new("components/onboarding.ice", 1938, 1, "rendered view node");
#[cfg(test)]
let __ice_render_source = ::ui_lang_runtime::testing::push_render_source(__ice_render_source_location);
let __ice_rendered: __IceElement<'_, __DucktapeMessage> = { let __a11y_key = format!("{}/@container:1938", __ice_component_04a6f696e53637265656e_scope_4021); let __container_content: __IceElement<'_, __DucktapeMessage> = {
#[cfg(test)]
let __ice_render_source_location = ::ui_lang_runtime::testing::Location::new("components/onboarding.ice", 1947, 1, "rendered view node");
#[cfg(test)]
let __ice_render_source = ::ui_lang_runtime::testing::push_render_source(__ice_render_source_location);
let __ice_rendered: __IceElement<'_, __DucktapeMessage> = (|| { let __slot_content: __IceElement<'_, __DucktapeMessage> = {
#[cfg(test)]
let __ice_render_source_location = ::ui_lang_runtime::testing::Location::new("components/onboarding.ice", 179, 1, "rendered view node");
#[cfg(test)]
let __ice_render_source = ::ui_lang_runtime::testing::push_render_source(__ice_render_source_location);
let __ice_rendered: __IceElement<'_, __DucktapeMessage> = (|| { let __slot_content: __IceElement<'_, __DucktapeMessage> = {
#[cfg(test)]
let __ice_render_source_location = ::ui_lang_runtime::testing::Location::new("view.ice", 73, 1, "rendered view node");
#[cfg(test)]
let __ice_render_source = ::ui_lang_runtime::testing::push_render_source(__ice_render_source_location);
let __ice_rendered: __IceElement<'_, __DucktapeMessage> = { let __ice_node_scope = format!("{}/join-invite", __ice_node_scope); { let __a11y_key = __ice_node_scope.clone(); let __a11y_id = ::ui_lang_runtime::StableId::new(&__a11y_key); let __disabled = (*self.__ice_derived_mutation_busy()); let __secure = true; let __role = if __secure { ::ui_lang_runtime::Role::PasswordInput } else { ::ui_lang_runtime::Role::TextInput }; let __input = ::ui_lang_runtime::accessible(::iced::widget::text_input("🦆AAAA…", &self.__ice_secrets.text("join_invite")).id(::iced::widget::Id::from(__a11y_key.clone())).padding(::iced::Padding { top: 11.0, right: 13.0, bottom: 11.0, left: 13.0 }).width(::iced::Fill).secure(__secure).width(::iced::Fill).padding(0.0 as f32).size(((12.0) as f32).max(f32::EPSILON).min(f32::MAX)).line_height(::iced::widget::text::LineHeight::Relative(((1.2) as f32).max(f32::EPSILON).min(f32::MAX))).font(::iced::Font { family: ::iced::font::Family::Name("Geist Mono"), weight: ::iced::font::Weight::Normal, stretch: ::iced::font::Stretch::Normal, style: ::iced::font::Style::Normal }).on_input_maybe(if __disabled { None } else { Some(move |__text| __DucktapeMessage::__SecretTyped(::std::string::String::from("join_invite"), __text)) }).on_submit_maybe(if __disabled { None } else { Some(__DucktapeMessage::JoinNetworkSubmit) }).style(move |__theme, __status| { let mut __style = ::iced::widget::text_input::default(__theme, __status); __style.background = __ice_palette.colors[3].into(); __style.border.color = __ice_palette.colors[39]; __style.border.width = 1.0; __style.border.radius = 10.0.into(); __style.background = ::iced::Background::Color(::iced::Color::from_rgba8(0, 0, 0, 0.000000)); __style.border.color = ::iced::Color::from_rgba8(0, 0, 0, 0.000000); __style.border.width = 0.0 as f32; __style.border.radius = ::iced::border::Radius { top_left: ((0.0) as f32).max(0.0).min(f32::MAX), top_right: ((0.0) as f32).max(0.0).min(f32::MAX), bottom_right: ((0.0) as f32).max(0.0).min(f32::MAX), bottom_left: ((0.0) as f32).max(0.0).min(f32::MAX) }; __style.placeholder = __ice_palette.colors[73]; __style.value = __ice_palette.colors[4]; __style.selection = { let mut __color = __ice_palette.colors[4]; __color.a = 0.180000; __color }; if matches!(__status, ::iced::widget::text_input::Status::Focused { .. }) { __style.border.color = __ice_palette.colors[42]; } match __status { ::iced::widget::text_input::Status::Disabled => { __style.value = __ice_palette.colors[72]; } _ => {} } __style }), __a11y_id, __role).logical_id_maybe(::core::cfg!(test).then_some(&*__a11y_key)).focus_id(::iced::widget::Id::from(__a11y_key)).label("Invite".to_owned()).value_maybe(::std::option::Option::None).disabled(__disabled); __input.into() } };
#[cfg(test)]
drop(__ice_render_source);
__ice_rendered
}; __slot_content })();
#[cfg(test)]
drop(__ice_render_source);
__ice_rendered
}; __slot_content })();
#[cfg(test)]
drop(__ice_render_source);
__ice_rendered
}; let __container = {  ::iced::widget::container(__container_content).padding(::ui_lang_runtime::bounded_padding(12.0, 14.0, 12.0, 14.0)).width(::iced::Fill).style(move |__theme| { let mut __style = ::iced::widget::container::Style::default(); __style.background = ::std::option::Option::Some(::iced::Background::Color(__ice_palette.colors[3])); __style.border.color = __ice_palette.colors[7]; __style.border.width = 1.5 as f32; __style.border.radius = ::iced::border::Radius { top_left: ((10.0) as f32).max(0.0).min(f32::MAX), top_right: ((10.0) as f32).max(0.0).min(f32::MAX), bottom_right: ((10.0) as f32).max(0.0).min(f32::MAX), bottom_left: ((10.0) as f32).max(0.0).min(f32::MAX) }; __style }) }; ::ui_lang_runtime::accessible(__container, ::ui_lang_runtime::StableId::new(&__a11y_key), ::ui_lang_runtime::Role::GenericContainer).logical_id_maybe(::core::cfg!(test).then_some(__a11y_key)).into() };
#[cfg(test)]
drop(__ice_render_source);
__ice_rendered
}; let __container = {  ::iced::widget::container(__container_content).padding(::ui_lang_runtime::bounded_padding(8.0, 0.0, 0.0, 0.0)).width(::iced::Fill) }; ::ui_lang_runtime::accessible(__container, ::ui_lang_runtime::StableId::new(&__a11y_key), ::ui_lang_runtime::Role::GenericContainer).logical_id_maybe(::core::cfg!(test).then_some(__a11y_key)).into() };
#[cfg(test)]
drop(__ice_render_source);
__ice_rendered
}); __children.push({
#[cfg(test)]
let __ice_render_source_location = ::ui_lang_runtime::testing::Location::new("components/onboarding.ice", 1948, 1, "rendered view node");
#[cfg(test)]
let __ice_render_source = ::ui_lang_runtime::testing::push_render_source(__ice_render_source_location);
let __ice_rendered: __IceElement<'_, __DucktapeMessage> = { let __a11y_key = format!("{}/@container:1948", __ice_component_04a6f696e53637265656e_scope_4021); let __container_content: __IceElement<'_, __DucktapeMessage> = {
#[cfg(test)]
let __ice_render_source_location = ::ui_lang_runtime::testing::Location::new("components/onboarding.ice", 1949, 1, "rendered view node");
#[cfg(test)]
let __ice_render_source = ::ui_lang_runtime::testing::push_render_source(__ice_render_source_location);
let __ice_rendered: __IceElement<'_, __DucktapeMessage> = { let __a11y_key = format!("{}/@container:1949", __ice_component_04a6f696e53637265656e_scope_4021); let __container_content: __IceElement<'_, __DucktapeMessage> = {
#[cfg(test)]
let __ice_render_source_location = ::ui_lang_runtime::testing::Location::new("components/onboarding.ice", 1957, 1, "rendered view node");
#[cfg(test)]
let __ice_render_source = ::ui_lang_runtime::testing::push_render_source(__ice_render_source_location);
let __ice_rendered: __IceElement<'_, __DucktapeMessage> = { let mut __children: ::std::vec::Vec<__IceElement<'_, __DucktapeMessage>> = ::std::vec::Vec::new(); __children.push({
#[cfg(test)]
let __ice_render_source_location = ::ui_lang_runtime::testing::Location::new("components/onboarding.ice", 1958, 1, "rendered view node");
#[cfg(test)]
let __ice_render_source = ::ui_lang_runtime::testing::push_render_source(__ice_render_source_location);
let __ice_rendered: __IceElement<'_, __DucktapeMessage> = { let __ice_component_04a6f696e5068617365526f77_scope_5809 = format!("{}/JoinPhaseRow@5809", __ice_component_04a6f696e53637265656e_scope_4021); ::ui_lang_runtime::rev_memo(895u64, (__ice_palette.name), ::ui_lang_runtime::grow_stack(|| self.__ice_component_use_36(__ice_palette, __ice_component_04a6f696e5068617365526f77_scope_5809))).into() };
#[cfg(test)]
drop(__ice_render_source);
__ice_rendered
}); __children.push({
#[cfg(test)]
let __ice_render_source_location = ::ui_lang_runtime::testing::Location::new("components/onboarding.ice", 1963, 1, "rendered view node");
#[cfg(test)]
let __ice_render_source = ::ui_lang_runtime::testing::push_render_source(__ice_render_source_location);
let __ice_rendered: __IceElement<'_, __DucktapeMessage> = { let __ice_component_04a6f696e5068617365526f77_scope_5814 = format!("{}/JoinPhaseRow@5814", __ice_component_04a6f696e53637265656e_scope_4021); ::ui_lang_runtime::rev_memo(896u64, (__ice_palette.name), ::ui_lang_runtime::grow_stack(|| self.__ice_component_use_37(__ice_palette, __ice_component_04a6f696e5068617365526f77_scope_5814))).into() };
#[cfg(test)]
drop(__ice_render_source);
__ice_rendered
}); __children.push({
#[cfg(test)]
let __ice_render_source_location = ::ui_lang_runtime::testing::Location::new("components/onboarding.ice", 1968, 1, "rendered view node");
#[cfg(test)]
let __ice_render_source = ::ui_lang_runtime::testing::push_render_source(__ice_render_source_location);
let __ice_rendered: __IceElement<'_, __DucktapeMessage> = { let __ice_component_04a6f696e5068617365526f77_scope_5819 = format!("{}/JoinPhaseRow@5819", __ice_component_04a6f696e53637265656e_scope_4021); ::ui_lang_runtime::rev_memo(897u64, (__ice_palette.name), ::ui_lang_runtime::grow_stack(|| self.__ice_component_use_38(__ice_palette, __ice_component_04a6f696e5068617365526f77_scope_5819))).into() };
#[cfg(test)]
drop(__ice_render_source);
__ice_rendered
}); __children.push({
#[cfg(test)]
let __ice_render_source_location = ::ui_lang_runtime::testing::Location::new("components/onboarding.ice", 1973, 1, "rendered view node");
#[cfg(test)]
let __ice_render_source = ::ui_lang_runtime::testing::push_render_source(__ice_render_source_location);
let __ice_rendered: __IceElement<'_, __DucktapeMessage> = { let __ice_component_04a6f696e5068617365526f77_scope_5824 = format!("{}/JoinPhaseRow@5824", __ice_component_04a6f696e53637265656e_scope_4021); ::ui_lang_runtime::rev_memo(898u64, (__ice_palette.name), ::ui_lang_runtime::grow_stack(|| self.__ice_component_use_39(__ice_palette, __ice_component_04a6f696e5068617365526f77_scope_5824))).into() };
#[cfg(test)]
drop(__ice_render_source);
__ice_rendered
}); let __child_count = __children.len(); let __children = __children.into_iter().map(|__child| ::ui_lang_runtime::bounded_fill_element(__child, __child_count, false)).collect::<::std::vec::Vec<_>>(); let __layout = ::iced::widget::column(__children).spacing(::ui_lang_runtime::bounded_spacing(10.0, __child_count)).width(::iced::Fill); let __content = ::iced::widget::container(__layout); let __layout_content: __IceElement<'_, __DucktapeMessage> = __content.into(); let __a11y_key = format!("{}/@layout:1957", __ice_component_04a6f696e53637265656e_scope_4021); ::ui_lang_runtime::accessible(__layout_content, ::ui_lang_runtime::StableId::new(&__a11y_key), ::ui_lang_runtime::Role::GenericContainer).logical_id_maybe(::core::cfg!(test).then_some(__a11y_key)).into() };
#[cfg(test)]
drop(__ice_render_source);
__ice_rendered
}; let __container = {  ::iced::widget::container(__container_content).padding(::ui_lang_runtime::bounded_padding(13.0, 13.0, 13.0, 13.0)).width(::iced::Fill).style(move |__theme| { let mut __style = ::iced::widget::container::Style::default(); __style.background = ::std::option::Option::Some(::iced::Background::Color(__ice_palette.colors[6])); __style.border.color = __ice_palette.colors[60]; __style.border.width = 1.0 as f32; __style.border.radius = ::iced::border::Radius { top_left: ((10.0) as f32).max(0.0).min(f32::MAX), top_right: ((10.0) as f32).max(0.0).min(f32::MAX), bottom_right: ((10.0) as f32).max(0.0).min(f32::MAX), bottom_left: ((10.0) as f32).max(0.0).min(f32::MAX) }; __style }) }; ::ui_lang_runtime::accessible(__container, ::ui_lang_runtime::StableId::new(&__a11y_key), ::ui_lang_runtime::Role::GenericContainer).logical_id_maybe(::core::cfg!(test).then_some(__a11y_key)).into() };
#[cfg(test)]
drop(__ice_render_source);
__ice_rendered
}; let __container = {  ::iced::widget::container(__container_content).padding(::ui_lang_runtime::bounded_padding(18.0, 0.0, 0.0, 0.0)).width(::iced::Fill) }; ::ui_lang_runtime::accessible(__container, ::ui_lang_runtime::StableId::new(&__a11y_key), ::ui_lang_runtime::Role::GenericContainer).logical_id_maybe(::core::cfg!(test).then_some(__a11y_key)).into() };
#[cfg(test)]
drop(__ice_render_source);
__ice_rendered
}); __children.push({
#[cfg(test)]
let __ice_render_source_location = ::ui_lang_runtime::testing::Location::new("components/onboarding.ice", 1978, 1, "rendered view node");
#[cfg(test)]
let __ice_render_source = ::ui_lang_runtime::testing::push_render_source(__ice_render_source_location);
let __ice_rendered: __IceElement<'_, __DucktapeMessage> = { let __a11y_key = format!("{}/@container:1978", __ice_component_04a6f696e53637265656e_scope_4021); let __container_content: __IceElement<'_, __DucktapeMessage> = {
#[cfg(test)]
let __ice_render_source_location = ::ui_lang_runtime::testing::Location::new("components/onboarding.ice", 1979, 1, "rendered view node");
#[cfg(test)]
let __ice_render_source = ::ui_lang_runtime::testing::push_render_source(__ice_render_source_location);
let __ice_rendered: __IceElement<'_, __DucktapeMessage> = { let __a11y_key = format!("{}/@button:1979", __ice_component_04a6f696e53637265656e_scope_4021); let __a11y_id = ::ui_lang_runtime::StableId::new(&__a11y_key); let __disabled = ((*self.__ice_derived_hub_busy()) || (self.__ice_secrets.text("join_invite")).is_empty()); let __activate = (__ice_cb_11)(); let __button_content: __IceElement<'_, __DucktapeMessage> = {
#[cfg(test)]
let __ice_render_source_location = ::ui_lang_runtime::testing::Location::new("components/onboarding.ice", 1988, 1, "rendered view node");
#[cfg(test)]
let __ice_render_source = ::ui_lang_runtime::testing::push_render_source(__ice_render_source_location);
let __ice_rendered: __IceElement<'_, __DucktapeMessage> = { let __a11y_key = format!("{}/@text:1988", __ice_component_04a6f696e53637265656e_scope_4021); let __text_value = ("Join →").to_string(); let __text = ::iced::widget::text(__text_value.clone()).size(((13.5) as f32).max(f32::EPSILON).min(f32::MAX)).width(::iced::Fill).align_x(::iced::widget::text::Alignment::Center).wrapping(::iced::widget::text::Wrapping::None).font(::iced::Font { family: ::iced::font::Family::Name("Geist"), weight: ::iced::font::Weight::Semibold, stretch: ::iced::font::Stretch::Normal, style: ::iced::font::Style::Normal }).color(__ice_palette.colors[9]); let __text = ::ui_lang_runtime::selectable_text(__text); ::ui_lang_runtime::accessible(__text, ::ui_lang_runtime::StableId::new(&__a11y_key), ::ui_lang_runtime::Role::Label).logical_id_maybe(::core::cfg!(test).then_some(__a11y_key)).value(__text_value).into() };
#[cfg(test)]
drop(__ice_render_source);
__ice_rendered
}; let __button = ::iced::widget::button(__button_content).padding(::iced::Padding { top: 13.0, right: 0.0, bottom: 13.0, left: 0.0 }).width(::iced::Fill).on_press_maybe(if __disabled { None } else { Some(__activate.clone()) }).style(move |__theme, __status| { let mut __style = ::iced::widget::button::primary(__theme, __status); let __background: Option<::iced::Color> = match __status { ::iced::widget::button::Status::Hovered => Some(__ice_palette.colors[8]), ::iced::widget::button::Status::Pressed => Some({ let mut __color = __ice_palette.colors[7]; __color.a = 0.800000; __color }), ::iced::widget::button::Status::Disabled => Some(__ice_palette.colors[7]), _ => Some(__ice_palette.colors[7]) }; if let Some(__background) = __background { __style.background = Some(::iced::Background::Color(__background)); } __style.text_color = __ice_palette.colors[9]; __style.border.radius = 10.0.into(); if matches!(__status, ::iced::widget::button::Status::Disabled) { __style.background = Some(__ice_palette.colors[10].into()); __style.text_color = __ice_palette.colors[11]; } __style }); ::ui_lang_runtime::accessible(__button, __a11y_id, ::ui_lang_runtime::Role::Button).logical_id_maybe(::core::cfg!(test).then_some(&*__a11y_key)).focus_id(::iced::widget::Id::from(__a11y_key)).label("Join network".to_owned()).disabled(__disabled).on_activate_maybe(if __disabled { None } else { Some(__activate) }).focus_ring(__ice_palette.colors[42], 10.0).into() };
#[cfg(test)]
drop(__ice_render_source);
__ice_rendered
}; let __container = {  ::iced::widget::container(__container_content).padding(::ui_lang_runtime::bounded_padding(22.0, 0.0, 0.0, 0.0)).width(::iced::Fill) }; ::ui_lang_runtime::accessible(__container, ::ui_lang_runtime::StableId::new(&__a11y_key), ::ui_lang_runtime::Role::GenericContainer).logical_id_maybe(::core::cfg!(test).then_some(__a11y_key)).into() };
#[cfg(test)]
drop(__ice_render_source);
__ice_rendered
}); __children.push({
#[cfg(test)]
let __ice_render_source_location = ::ui_lang_runtime::testing::Location::new("components/onboarding.ice", 1996, 1, "rendered view node");
#[cfg(test)]
let __ice_render_source = ::ui_lang_runtime::testing::push_render_source(__ice_render_source_location);
let __ice_rendered: __IceElement<'_, __DucktapeMessage> = { let __ice_component_04f6e626f617264696e674572726f72_scope_5847 = format!("{}/OnboardingError@5847", __ice_component_04a6f696e53637265656e_scope_4021); ::ui_lang_runtime::rev_memo(902u64, (self.__ice_rev[131], __ice_palette.name), ::ui_lang_runtime::grow_stack(|| self.__ice_component_use_5(__ice_palette, __ice_component_04f6e626f617264696e674572726f72_scope_5847))).into() };
#[cfg(test)]
drop(__ice_render_source);
__ice_rendered
}); let __child_count = __children.len(); let __children = __children.into_iter().map(|__child| ::ui_lang_runtime::bounded_fill_element(__child, __child_count, false)).collect::<::std::vec::Vec<_>>(); let __layout = ::iced::widget::column(__children).spacing(::ui_lang_runtime::bounded_spacing(0.0, __child_count)).width(428.0 as f32); let __content = ::iced::widget::container(__layout); let __layout_content: __IceElement<'_, __DucktapeMessage> = __content.into(); let __a11y_key = __ice_node_scope.as_str(); ::ui_lang_runtime::accessible(__layout_content, ::ui_lang_runtime::StableId::new(&__a11y_key), ::ui_lang_runtime::Role::GenericContainer).logical_id_maybe(::core::cfg!(test).then_some(__a11y_key)).into() } };
#[cfg(test)]
drop(__ice_render_source);
__ice_rendered
}; __component_content };
#[cfg(test)]
drop(__ice_render_source);
__ice_rendered
}); }, HubStep::Loading => { __children.push({
#[cfg(test)]
let __ice_render_source_location = ::ui_lang_runtime::testing::Location::new("components/onboarding.ice", 181, 1, "rendered view node");
#[cfg(test)]
let __ice_render_source = ::ui_lang_runtime::testing::push_render_source(__ice_render_source_location);
let __ice_rendered: __IceElement<'_, __DucktapeMessage> = { let mut __children: ::std::vec::Vec<__IceElement<'_, __DucktapeMessage>> = ::std::vec::Vec::new(); __children.push({
#[cfg(test)]
let __ice_render_source_location = ::ui_lang_runtime::testing::Location::new("components/onboarding.ice", 182, 1, "rendered view node");
#[cfg(test)]
let __ice_render_source = ::ui_lang_runtime::testing::push_render_source(__ice_render_source_location);
let __ice_rendered: __IceElement<'_, __DucktapeMessage> = { let __a11y_key = format!("{}/@text:182", __ice_component_0487562436f6c756d6e_scope_10219); let __text_value = ("…").to_string(); let __text = ::iced::widget::text(__text_value.clone()).size(((13.5) as f32).max(f32::EPSILON).min(f32::MAX)).wrapping(::iced::widget::text::Wrapping::None).color(__ice_palette.colors[72]); let __text = ::ui_lang_runtime::selectable_text(__text); ::ui_lang_runtime::accessible(__text, ::ui_lang_runtime::StableId::new(&__a11y_key), ::ui_lang_runtime::Role::Label).logical_id_maybe(::core::cfg!(test).then_some(__a11y_key)).value(__text_value).into() };
#[cfg(test)]
drop(__ice_render_source);
__ice_rendered
}); let __child_count = __children.len(); let __children = __children.into_iter().map(|__child| ::ui_lang_runtime::bounded_fill_element(__child, __child_count, false)).collect::<::std::vec::Vec<_>>(); let __layout = ::iced::widget::column(__children).spacing(::ui_lang_runtime::bounded_spacing(0.0, __child_count)).align_x(::iced::alignment::Horizontal::Center); let __content = ::iced::widget::container(__layout); let __layout_content: __IceElement<'_, __DucktapeMessage> = __content.into(); let __a11y_key = format!("{}/@layout:181", __ice_component_0487562436f6c756d6e_scope_10219); ::ui_lang_runtime::accessible(__layout_content, ::ui_lang_runtime::StableId::new(&__a11y_key), ::ui_lang_runtime::Role::GenericContainer).logical_id_maybe(::core::cfg!(test).then_some(__a11y_key)).into() };
#[cfg(test)]
drop(__ice_render_source);
__ice_rendered
}); }, HubStep::Account => { __children.push({
#[cfg(test)]
let __ice_render_source_location = ::ui_lang_runtime::testing::Location::new("components/onboarding.ice", 188, 1, "rendered view node");
#[cfg(test)]
let __ice_render_source = ::ui_lang_runtime::testing::push_render_source(__ice_render_source_location);
let __ice_rendered: __IceElement<'_, __DucktapeMessage> = { let __ice_node_scope = format!("{}/welcome", __ice_node_scope); { let __ice_component_057656c636f6d6553637265656e_scope_4039 = __ice_node_scope.clone(); ::ui_lang_runtime::rev_memo(514u64, (self.__ice_rev[124], self.__ice_rev[131], self.__ice_rev[132], self.__ice_rev[137], self.__ice_rev[138], self.__ice_rev[139], self.__ice_rev[140], self.__ice_rev[141], self.__ice_rev[19], __ice_palette.name), ::ui_lang_runtime::grow_stack(|| self.__ice_component_use_41(__ice_palette, __ice_component_057656c636f6d6553637265656e_scope_4039, (move || __DucktapeMessage::WelcomeCancel).clone(), (move |__event_0| __DucktapeMessage::WelcomeCreateSubmit(__event_0)).clone(), (move || __DucktapeMessage::WelcomeDesktop).clone(), (move || __DucktapeMessage::WelcomeLoginSubmit).clone(), (move || __DucktapeMessage::WelcomeSkip).clone()))).into() } };
#[cfg(test)]
drop(__ice_render_source);
__ice_rendered
}); }, } let __child_count = __children.len(); let __children = __children.into_iter().map(|__child| ::ui_lang_runtime::bounded_fill_element(__child, __child_count, false)).collect::<::std::vec::Vec<_>>(); let __layout = ::iced::widget::column(__children).spacing(::ui_lang_runtime::bounded_spacing(0.0, __child_count)); let __content = ::iced::widget::container(__layout); let __layout_content: __IceElement<'_, __DucktapeMessage> = __content.into(); let __a11y_key = format!("{}/@layout:84", __ice_component_0487562436f6c756d6e_scope_10219); ::ui_lang_runtime::accessible(__layout_content, ::ui_lang_runtime::StableId::new(&__a11y_key), ::ui_lang_runtime::Role::GenericContainer).logical_id_maybe(::core::cfg!(test).then_some(__a11y_key)).into() };
#[cfg(test)]
drop(__ice_render_source);
__ice_rendered
}; let __container = {  ::iced::widget::container(__container_content).padding(::ui_lang_runtime::bounded_padding(0.0, 26.0, 26.0, 26.0)).width(::iced::Fill).height(::iced::Fill).align_x(::iced::alignment::Horizontal::Center).align_y(::iced::alignment::Vertical::Center) }; ::ui_lang_runtime::accessible(__container, ::ui_lang_runtime::StableId::new(&__a11y_key), ::ui_lang_runtime::Role::GenericContainer).logical_id_maybe(::core::cfg!(test).then_some(__a11y_key)).into() };
#[cfg(test)]
drop(__ice_render_source);
__ice_rendered
}); let __child_count = __children.len(); let __children = __children.into_iter().map(|__child| ::ui_lang_runtime::bounded_fill_element(__child, __child_count, false)).collect::<::std::vec::Vec<_>>(); let __layout = ::iced::widget::column(__children).width(::iced::Fill).height(::iced::Fill); let __content = ::iced::widget::container(__layout); let __layout_content: __IceElement<'_, __DucktapeMessage> = __content.into(); let __a11y_key = format!("{}/@layout:52", __ice_component_0487562436f6c756d6e_scope_10219); ::ui_lang_runtime::accessible(__layout_content, ::ui_lang_runtime::StableId::new(&__a11y_key), ::ui_lang_runtime::Role::GenericContainer).logical_id_maybe(::core::cfg!(test).then_some(__a11y_key)).into() };
#[cfg(test)]
drop(__ice_render_source);
__ice_rendered
}; let __container = {  ::iced::widget::container(__container_content).id(::iced::widget::Id::from(__ice_node_scope.clone())).width(::iced::Fill).height(::iced::Fill).style(move |__theme| { let mut __style = ::iced::widget::container::Style::default(); __style.background = ::std::option::Option::Some(::iced::Background::Color(__ice_palette.colors[86])); __style }) }; ::ui_lang_runtime::accessible(__container, ::ui_lang_runtime::StableId::new(&__a11y_key), ::ui_lang_runtime::Role::GenericContainer).logical_id_maybe(::core::cfg!(test).then_some(__a11y_key)).into() } };
#[cfg(test)]
drop(__ice_render_source);
__ice_rendered
}; __component_content };
#[cfg(test)]
drop(__ice_render_source);
__ice_rendered
}); } __children });__ice_slots.push_group({ let mut __children: ::std::vec::Vec<__IceElement<'_, __DucktapeMessage>> = ::std::vec::Vec::new(); if (self.huddle_win == ::std::option::Option::Some(window)) { __children.push({
#[cfg(test)]
let __ice_render_source_location = ::ui_lang_runtime::testing::Location::new("view.ice", 91, 1, "rendered view node");
#[cfg(test)]
let __ice_render_source = ::ui_lang_runtime::testing::push_render_source(__ice_render_source_location);
let __ice_rendered: __IceElement<'_, __DucktapeMessage> = { let __ice_node_scope = format!("{}/huddle", "Ducktape"); ::ui_lang_runtime::grow_stack(|| self.__ice_component_use_62(__ice_palette, __ice_node_scope.clone())) };
#[cfg(test)]
drop(__ice_render_source);
__ice_rendered
}); } __children });__ice_slots.push_group({ let mut __children: ::std::vec::Vec<__IceElement<'_, __DucktapeMessage>> = ::std::vec::Vec::new(); if (self.console_win == ::std::option::Option::Some(window)) { __children.push({
#[cfg(test)]
let __ice_render_source_location = ::ui_lang_runtime::testing::Location::new("view.ice", 109, 1, "rendered view node");
#[cfg(test)]
let __ice_render_source = ::ui_lang_runtime::testing::push_render_source(__ice_render_source_location);
let __ice_rendered: __IceElement<'_, __DucktapeMessage> = { let __ice_node_scope = format!("{}/workspace-tabs", "Ducktape"); ::ui_lang_runtime::grow_stack(|| self.__ice_component_use_96(__ice_palette, __ice_node_scope.clone())) };
#[cfg(test)]
drop(__ice_render_source);
__ice_rendered
}); } __children }); }

}
}
