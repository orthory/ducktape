//! A QR code that owns its data.
//!
//! [`iced::widget::qr_code`] borrows a `qr_code::Data` that must outlive the
//! whole view, so it can only render a matrix stored in application state. A
//! payload minted while the view is being built — an invite link, a signed
//! URL — has no such home, so this widget owns the `Data` and hands the
//! borrowed iced widget a reference to its own field on every call.
use iced::advanced::widget::{Tree, tree};
use iced::advanced::{Layout, Widget, layout, mouse, renderer};
use iced::widget::qr_code::{Data, QRCode, Style};
use iced::{Element, Length, Rectangle, Renderer, Size, Theme};

/// Creates a QR code from an owned matrix.
///
/// `data` is `None` when the payload could not be encoded — it exceeded the
/// capacity of the requested version and error correction — in which case the
/// widget occupies no space and draws nothing. A literal payload is rejected
/// by the Ice checker instead, long before this can happen.
pub fn qr_code(data: Option<Data>) -> QrCode {
    QrCode {
        data,
        cell_size: None,
        total_size: None,
        style: None,
    }
}

type StyleFn = dyn Fn(&Theme) -> Style;

/// A QR code widget that owns the matrix it draws.
pub struct QrCode {
    data: Option<Data>,
    cell_size: Option<f32>,
    total_size: Option<f32>,
    style: Option<Box<StyleFn>>,
}

impl QrCode {
    /// Sets the size of the individual cells of the QR code.
    #[must_use]
    pub fn cell_size(mut self, cell_size: f32) -> Self {
        self.cell_size = Some(cell_size);
        self
    }

    /// Sets the size of the whole QR code.
    #[must_use]
    pub fn total_size(mut self, total_size: f32) -> Self {
        self.total_size = Some(total_size);
        self
    }

    /// Sets the cell and background colors of the QR code.
    #[must_use]
    pub fn style(mut self, style: impl Fn(&Theme) -> Style + 'static) -> Self {
        self.style = Some(Box::new(style));
        self
    }

    fn borrowed(&self) -> Option<QRCode<'_, Theme>> {
        let mut code = QRCode::new(self.data.as_ref()?);
        if let Some(cell_size) = self.cell_size {
            code = code.cell_size(cell_size);
        }
        if let Some(total_size) = self.total_size {
            code = code.total_size(total_size);
        }
        if let Some(style) = &self.style {
            code = code.style(|theme| style(theme));
        }
        Some(code)
    }
}

impl<Message> Widget<Message, Theme, Renderer> for QrCode {
    fn tag(&self) -> tree::Tag {
        self.borrowed().map_or_else(tree::Tag::stateless, |code| {
            Widget::<Message, Theme, Renderer>::tag(&code)
        })
    }

    fn state(&self) -> tree::State {
        self.borrowed().map_or_else(
            || tree::State::None,
            |code| Widget::<Message, Theme, Renderer>::state(&code),
        )
    }

    fn size(&self) -> Size<Length> {
        Size {
            width: Length::Shrink,
            height: Length::Shrink,
        }
    }

    fn layout(
        &mut self,
        tree: &mut Tree,
        renderer: &Renderer,
        limits: &layout::Limits,
    ) -> layout::Node {
        let Some(mut code) = self.borrowed() else {
            return layout::Node::new(Size::ZERO);
        };
        Widget::<Message, Theme, Renderer>::layout(&mut code, tree, renderer, limits)
    }

    fn draw(
        &self,
        tree: &Tree,
        renderer: &mut Renderer,
        theme: &Theme,
        style: &renderer::Style,
        layout: Layout<'_>,
        cursor: mouse::Cursor,
        viewport: &Rectangle,
    ) {
        let Some(code) = self.borrowed() else {
            return;
        };
        Widget::<Message, Theme, Renderer>::draw(
            &code, tree, renderer, theme, style, layout, cursor, viewport,
        );
    }
}

impl<'a, Message: 'a> From<QrCode> for Element<'a, Message> {
    fn from(code: QrCode) -> Self {
        Self::new(code)
    }
}
