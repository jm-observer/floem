use std::cell::{Cell, RefCell};
use std::rc::Rc;
use std::sync::Arc;
use floem_editor_core::buffer::rope_text::RopeText;
use floem_reactive::{RwSignal, SignalGet, SignalWith};
use crate::views::editor::{Editor, EditorFontSizes};
use crate::views::editor::layout::TextLayoutLine;
use crate::views::editor::view::{DiffSection, ScreenLinesBase};
use crate::views::editor::visual_line::{ResolvedWrap, TextLayoutCache, TextLayoutProvider};

pub struct OriginLine {
    line_index: usize,
}

pub struct OriginFoldedLine {
    line_index: usize,
    // [origin_line_start..origin_line_end)
    origin_line_start: usize,
    origin_line_end: usize,
    text_layout: Arc<TextLayoutLine>,
}

pub struct VisualLine {
    line_index: usize,
    origin_folded_line: usize,
    origin_folded_line_sub_index: usize,
}

pub struct Lines {
    origin_lines: Vec<OriginLine>,
    origin_folded_lines: Vec<OriginFoldedLine>,
    visual_liens: Vec<VisualLine>,
    pub font_sizes: RefCell<Rc<EditorFontSizes>>,
    wrap: Cell<ResolvedWrap>,
}

impl Lines {
    fn update(&mut self, editor: &Editor) {
        let last_line = self.font_sizes.borrow().doc.get_untracked().rope_text().last_line();
        let mut current_line = 0;
        // while current_line <= last_line {
        //     let text_layout = editor.new_text_layout(current_line);
        //     text_layout.phantom_text.v
        // }
    }
}