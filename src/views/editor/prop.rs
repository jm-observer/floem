use peniko::{Brush, Color};
use floem_editor_core::indent::IndentStyle;
use crate::{IntoView, prop, prop_extractor, View};
use crate::style::{CursorColor, StylePropValue, TextColor};
use crate::views::editor::text::{RenderWhitespace, WrapMethod};
use crate::views::text;
prop!(pub WrapProp: WrapMethod {} = WrapMethod::EditorWidth);
impl StylePropValue for WrapMethod {
    fn debug_view(&self) -> Option<Box<dyn View>> {
        Some(crate::views::text(self).into_any())
    }
}
prop!(pub CursorSurroundingLines: usize {} = 1);
prop!(pub ScrollBeyondLastLine: bool {} = false);
prop!(pub ShowIndentGuide: bool {} = false);
prop!(pub Modal: bool {} = false);
prop!(pub ModalRelativeLine: bool {} = false);
prop!(pub SmartTab: bool {} = false);
prop!(pub PhantomColor: Color {} = Color::DIM_GRAY);
prop!(pub PlaceholderColor: Color {} = Color::DIM_GRAY);
prop!(pub PreeditUnderlineColor: Color {} = Color::WHITE);
prop!(pub RenderWhitespaceProp: RenderWhitespace {} = RenderWhitespace::None);
impl StylePropValue for RenderWhitespace {
    fn debug_view(&self) -> Option<Box<dyn View>> {
        Some(crate::views::text(self).into_any())
    }
}
prop!(pub IndentStyleProp: IndentStyle {} = IndentStyle::Spaces(4));
impl StylePropValue for IndentStyle {
    fn debug_view(&self) -> Option<Box<dyn View>> {
        Some(text(self).into_any())
    }
}
prop!(pub DropdownShadow: Option<Color> {} = None);
prop!(pub Foreground: Color { inherited } = Color::rgb8(0x38, 0x3A, 0x42));
prop!(pub Focus: Option<Color> {} = None);
prop!(pub SelectionColor: Color {} = Color::BLACK.with_alpha_factor(0.5));
prop!(pub CurrentLineColor: Option<Color> {  } = None);
prop!(pub Link: Option<Color> {} = None);
prop!(pub VisibleWhitespaceColor: Color {} = Color::TRANSPARENT);
prop!(pub IndentGuideColor: Color {} = Color::TRANSPARENT);
prop!(pub StickyHeaderBackground: Option<Color> {} = None);

prop_extractor! {
    pub EditorStyle {
        pub text_color: TextColor,
        pub phantom_color: PhantomColor,
        pub placeholder_color: PlaceholderColor,
        pub preedit_underline_color: PreeditUnderlineColor,
        pub show_indent_guide: ShowIndentGuide,
        pub modal: Modal,
        // Whether line numbers are relative in modal mode
        pub modal_relative_line: ModalRelativeLine,
        // Whether to insert the indent that is detected for the file when a tab character
        // is inputted.
        pub smart_tab: SmartTab,
        pub wrap_method: WrapProp,
        pub cursor_surrounding_lines: CursorSurroundingLines,
        pub render_whitespace: RenderWhitespaceProp,
        pub indent_style: IndentStyleProp,
        pub caret: CursorColor,
        pub selection: SelectionColor,
        pub current_line: CurrentLineColor,
        pub visible_whitespace: VisibleWhitespaceColor,
        pub indent_guide: IndentGuideColor,
        pub scroll_beyond_last_line: ScrollBeyondLastLine,
    }
}
impl crate::views::editor::EditorStyle {
    pub fn ed_text_color(&self) -> Color {
        self.text_color().unwrap_or(Color::BLACK)
    }
}
impl crate::views::editor::EditorStyle {
    pub fn ed_caret(&self) -> Brush {
        self.caret()
    }
}