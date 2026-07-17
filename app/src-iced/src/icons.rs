use iced::widget::{Svg, svg};
use iced::{Color, Length};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Icon {
    Agent,
    Bell,
    Browser,
    Chat,
    ChevronLeft,
    ChevronRight,
    Close,
    Explorer,
    Files,
    Forge,
    Governance,
    Home,
    Link,
    Maximize,
    Members,
    Metrics,
    Minimize,
    Modules,
    Moon,
    Node,
    Pages,
    Plus,
    Refresh,
    Sandbox,
    Search,
    Settings,
    Sun,
    Terminal,
}

impl Icon {
    fn body(self) -> &'static str {
        match self {
            Self::Agent => {
                "<path d='M12 2a4 4 0 0 0-4 4v1H6a3 3 0 0 0-3 3v6a3 3 0 0 0 3 3h1v3l4-3h7a3 3 0 0 0 3-3v-6a3 3 0 0 0-3-3h-2V6a4 4 0 0 0-4-4Z'/><circle cx='9' cy='13' r='1'/><circle cx='15' cy='13' r='1'/>"
            }
            Self::Bell => {
                "<path d='M18 8a6 6 0 0 0-12 0c0 7-3 7-3 9h18c0-2-3-2-3-9'/><path d='M10 21h4'/>"
            }
            Self::Browser => {
                "<rect x='3' y='4' width='18' height='16' rx='2'/><path d='M3 9h18'/><circle cx='7' cy='6.5' r='.5'/><circle cx='10' cy='6.5' r='.5'/>"
            }
            Self::Chat => {
                "<path d='M21 15a4 4 0 0 1-4 4H8l-5 3V7a4 4 0 0 1 4-4h10a4 4 0 0 1 4 4Z'/><path d='M8 9h8M8 13h5'/>"
            }
            Self::ChevronLeft => "<path d='m15 18-6-6 6-6'/>",
            Self::ChevronRight => "<path d='m9 18 6-6-6-6'/>",
            Self::Close => "<path d='M6 6l12 12M18 6 6 18'/>",
            Self::Explorer => "<circle cx='11' cy='11' r='7'/><path d='m20 20-4-4'/>",
            Self::Files => {
                "<path d='M14 2H6a2 2 0 0 0-2 2v16a2 2 0 0 0 2 2h12a2 2 0 0 0 2-2V8Z'/><path d='M14 2v6h6'/>"
            }
            Self::Forge => "<path d='M14 6 4 16v4h4L18 10'/><path d='m16 4 4 4-3 3-4-4Z'/>",
            Self::Governance => {
                "<path d='m3 10 9-7 9 7'/><path d='M5 10h14M6 10v8m4-8v8m4-8v8m4-8v8M3 21h18'/>"
            }
            Self::Home => "<path d='m3 11 9-8 9 8'/><path d='M5 10v10h14V10M9 20v-6h6v6'/>",
            Self::Link => {
                "<path d='M10 13a5 5 0 0 0 7.1.1l2-2a5 5 0 0 0-7.1-7.1l-1.1 1.1'/><path d='M14 11a5 5 0 0 0-7.1-.1l-2 2A5 5 0 0 0 12 20l1.1-1.1'/>"
            }
            Self::Maximize => "<rect x='6' y='6' width='12' height='12' rx='1'/>",
            Self::Members => {
                "<path d='M16 21v-2a4 4 0 0 0-4-4H6a4 4 0 0 0-4 4v2'/><circle cx='9' cy='7' r='4'/><path d='M22 21v-2a4 4 0 0 0-3-3.9M16 3.1a4 4 0 0 1 0 7.8'/>"
            }
            Self::Metrics => "<path d='M4 20V10M10 20V4M16 20v-7M22 20H2'/>",
            Self::Minimize => "<path d='M6 12h12'/>",
            Self::Modules => {
                "<rect x='3' y='3' width='7' height='7' rx='1'/><rect x='14' y='3' width='7' height='7' rx='1'/><rect x='3' y='14' width='7' height='7' rx='1'/><rect x='14' y='14' width='7' height='7' rx='1'/>"
            }
            Self::Moon => "<path d='M21 12.8A9 9 0 1 1 11.2 3 7 7 0 0 0 21 12.8Z'/>",
            Self::Node => {
                "<circle cx='12' cy='5' r='3'/><circle cx='5' cy='19' r='3'/><circle cx='19' cy='19' r='3'/><path d='m10.5 7.5-4 8M13.5 7.5l4 8M8 19h8'/>"
            }
            Self::Pages => "<path d='M5 3h10l4 4v14H5Z'/><path d='M15 3v5h4M8 12h8M8 16h8'/>",
            Self::Plus => "<path d='M12 5v14M5 12h14'/>",
            Self::Refresh => {
                "<path d='M21 12a9 9 0 1 1-2.6-6.4L21 8'/><path d='M21 3v5h-5'/>"
            }
            Self::Sandbox => "<path d='m12 2 9 5-9 5-9-5Z'/><path d='m3 12 9 5 9-5M3 17l9 5 9-5'/>",
            Self::Search => "<circle cx='11' cy='11' r='7'/><path d='m20 20-4-4'/>",
            Self::Settings => {
                "<circle cx='12' cy='12' r='3'/><path d='M19.4 15a1.7 1.7 0 0 0 .3 1.9l.1.1-2.8 2.8-.1-.1a1.7 1.7 0 0 0-1.9-.3 1.7 1.7 0 0 0-1 1.6v.2h-4V21a1.7 1.7 0 0 0-1-1.6 1.7 1.7 0 0 0-1.9.3l-.1.1L4.2 17l.1-.1a1.7 1.7 0 0 0 .3-1.9A1.7 1.7 0 0 0 3 14H2.8v-4H3a1.7 1.7 0 0 0 1.6-1 1.7 1.7 0 0 0-.3-1.9L4.2 7 7 4.2l.1.1A1.7 1.7 0 0 0 9 4.6 1.7 1.7 0 0 0 10 3V2.8h4V3a1.7 1.7 0 0 0 1 1.6 1.7 1.7 0 0 0 1.9-.3l.1-.1L19.8 7l-.1.1a1.7 1.7 0 0 0-.3 1.9 1.7 1.7 0 0 0 1.6 1h.2v4H21a1.7 1.7 0 0 0-1.6 1Z'/>"
            }
            Self::Sun => {
                "<circle cx='12' cy='12' r='4'/><path d='M12 2v2M12 20v2M4.9 4.9l1.4 1.4M17.7 17.7l1.4 1.4M2 12h2M20 12h2M4.9 19.1l1.4-1.4M17.7 6.3l1.4-1.4'/>"
            }
            Self::Terminal => "<path d='m6 8 4 4-4 4M13 16h5'/>",
        }
    }
}

pub fn view(icon: Icon, size: f32, color: Color) -> Svg<'static> {
    let document = format!(
        "<svg xmlns='http://www.w3.org/2000/svg' viewBox='0 0 24 24' fill='none' stroke='currentColor' stroke-width='1.7' stroke-linecap='round' stroke-linejoin='round'>{}</svg>",
        icon.body()
    );

    Svg::new(svg::Handle::from_memory(document.into_bytes()))
        .width(Length::Fixed(size))
        .height(Length::Fixed(size))
        .style(move |_, _| svg::Style { color: Some(color) })
}
