use std::rc::Rc;
use std::sync::Arc;
use lapce_xi_rope::Interval;

use floem_editor_core::buffer::rope_text::RopeText;
use floem_editor_core::cursor::CursorAffinity;
use floem_reactive::{Scope, SignalGet, SignalWith};

use crate::views::editor::{Editor, EditorFontSizes};
use crate::views::editor::layout::TextLayoutLine;
use crate::views::editor::listener::Listener;
use crate::views::editor::visual_line::{FontSizeCacheId, LayoutEvent, ResolvedWrap, RVLine, TextLayoutProvider, VLine, VLineInfo};

#[allow(dead_code)]
#[derive(Clone, Copy)]
pub struct OriginLine {
    line_index: usize,
    start_offset: usize,
}
#[allow(dead_code)]
#[derive(Clone)]
pub struct OriginFoldedLine {
    pub line_index: usize,
    // [origin_line_start..origin_line_end]
    pub origin_line_start: usize,
    pub origin_line_end: usize,
    origin_interval: Interval,
    pub text_layout: Arc<TextLayoutLine>,
}

#[derive(Clone)]
pub struct VisualLine {
    line_index: usize,
    origin_interval: Interval,
    origin_folded_line: usize,
    origin_folded_line_sub_index: usize,
}

impl VisualLine {
    pub fn rvline(&self) -> RVLine {
        RVLine {
            line: self.origin_folded_line,
            line_index: self.origin_folded_line_sub_index,
        }
    }

    pub fn vline(&self) -> VLine {
        VLine(self.line_index)
    }
}

impl From<&VisualLine> for RVLine {
    fn from(value: &VisualLine) -> Self {
        value.rvline()
    }
}
impl From<&VisualLine> for VLine {
    fn from(value: &VisualLine) -> Self {
        value.vline()
    }
}
#[derive(Clone)]
pub struct Lines {
    origin_lines: Vec<OriginLine>,
    origin_folded_lines: Vec<OriginFoldedLine>,
    visual_lines: Vec<VisualLine>,
    pub font_sizes: Rc<EditorFontSizes>,
    font_size_cache_id: FontSizeCacheId,
    wrap: ResolvedWrap,
    pub layout_event: Listener<LayoutEvent>,
}

impl Lines {
    pub fn new(cx: Scope, font_sizes: Rc<EditorFontSizes>) -> Self {
        let id = font_sizes.cache_id();
        Self {
            font_sizes,
            wrap: ResolvedWrap::None,
            font_size_cache_id: id,
            layout_event: Listener::new_empty(cx),
            origin_lines: vec![],
            origin_folded_lines: vec![],
            visual_lines: vec![],
        }
    }

    pub fn update_font_sizes(&mut self, font_sizes: Rc<EditorFontSizes>, editor: &Editor) {
        self.font_sizes = font_sizes;
        self.clear();
        self.update(editor)
    }

    fn clear(&mut self) {
        self.origin_lines.clear();
        self.origin_folded_lines.clear();
        self.visual_lines.clear();
    }
    fn update(&mut self, editor: &Editor) {
        let rope_text = self
            .font_sizes
            .doc
            .get_untracked()
            .rope_text();
        let last_line = rope_text
            .last_line();
        self.clear();
        let mut current_line = 0;
        let mut origin_folded_line_index = 0;
        let mut visual_line_index = 0;
        while current_line <= last_line {
            let text_layout = editor.new_text_layout(current_line);
            let origin_line_start = text_layout.phantom_text.line;
            let origin_line_end = text_layout.phantom_text.last_line;
            let total_wrapped_lines = text_layout.text.line_layout().len();
            

            for origin_line in origin_line_start..=origin_line_end {
                self.origin_lines.push(OriginLine {
                    line_index: origin_line,
                    start_offset: rope_text.offset_of_line(origin_line),
                });
            }
            let origin_interval = Interval { start: rope_text.offset_of_line(origin_line_start), end: rope_text.offset_of_line(origin_line_end + 1) };
            self.origin_folded_lines.push(OriginFoldedLine {
                line_index: origin_folded_line_index,
                origin_line_start,
                origin_line_end,
                origin_interval,
                text_layout,
            });
            for origin_folded_line_sub_index in 0..total_wrapped_lines {
                self.visual_lines.push(VisualLine {
                    line_index: visual_line_index,
                    origin_interval,
                    origin_folded_line: origin_folded_line_index,
                    origin_folded_line_sub_index,
                });
                visual_line_index += 1;
            }

            current_line = origin_line_end + 1;
            origin_folded_line_index += 1;
        }
    }

    pub fn wrap(&self) -> ResolvedWrap {
        self.wrap
    }

    /// Set the wrapping style
    ///
    /// Does nothing if the wrapping style is the same as the current one.
    /// Will trigger a clear of the text layouts if the wrapping style is different.
    pub fn set_wrap(&mut self, wrap: ResolvedWrap, editor: &Editor) {
        if wrap == self.wrap {
            return;
        }
        self.wrap = wrap;
        self.clear();
        self.update(editor);
    }

    pub fn max_width(&self) -> f64 {
        todo!()
    }

    pub fn text_layout_of_visual_line(
        &self,
        line: usize,
    ) -> Arc<TextLayoutLine> {
        self.origin_folded_lines[self.visual_lines[line].origin_folded_line].text_layout.clone()
    }

    // 原始行的第一个视觉行。原始行可能会有多个视觉行
    pub fn start_visual_line_of_origin_line(&self, origin_line: usize) -> &VisualLine {
        let folded_line = self.folded_line_of_origin_line(origin_line);
        self.start_visual_line_of_folded_line(folded_line.line_index)
    }

    pub fn start_visual_line_of_folded_line(&self, origin_folded_line: usize) -> &VisualLine {
        for visual_line in &self.visual_lines {
            if visual_line.origin_folded_line == origin_folded_line{
                return visual_line
            }
        }
        panic!()
    }

    pub fn folded_line_of_origin_line(&self, origin_line: usize) -> &OriginFoldedLine {
        for folded_line in &self.origin_folded_lines {
            if folded_line.origin_line_start <= origin_line || origin_line <= folded_line.origin_line_end {
                return folded_line
            }
        }
        panic!()
    }

    pub fn visual_line_of_folded_line_and_sub_index(&self, origin_folded_line: usize, sub_index: usize) -> &VisualLine {
        for visual_line in &self.visual_lines {
            if visual_line.origin_folded_line == origin_folded_line && visual_line.origin_folded_line_sub_index == sub_index{
                return visual_line
            }
        }
        panic!()
    }

    pub fn last_visual_line(&self) -> &VisualLine {
        &self.visual_lines[self.visual_lines.len() - 1]
    }

    pub fn visual_line_of_offset(&self, offset: usize, _affinity: CursorAffinity) -> (RVLine, VLine, usize) {
        // 位于的原始行，以及在原始行的起始offset
        let (origin_line, offset_of_line) = self.font_sizes.doc.with_untracked(|x| {
            let text = x.text();
            let origin_line = text.line_of_offset(offset);
            let origin_line_start_offset = text.offset_of_line(origin_line);
            (origin_line, origin_line_start_offset)
        });
        let mut offset = offset - offset_of_line;
        let folded_line = self.folded_line_of_origin_line(origin_line);
        let folded_line_layout = folded_line.text_layout.text.line_layout();
        let mut sub_line_index = folded_line_layout.len() - 1;
        for (index, sub_line) in folded_line_layout.iter().enumerate() {
            if offset < sub_line.glyphs.len() {
                sub_line_index = index;
                break;
            } else {
                offset -= sub_line.glyphs.len();
            }
        }
        let visual_line = self.visual_line_of_folded_line_and_sub_index(folded_line.line_index, sub_line_index);
        (RVLine {
            line: folded_line.line_index,
            line_index: sub_line_index,
        }, VLine(visual_line.line_index), offset)
    }

    pub fn vline_infos(&self, start: usize, end: usize) -> Vec<VLineInfo<VLine>> {

        let mut vline_infos = Vec::with_capacity(end - start + 1);
        for index in start..=end {
            let rvline = self.visual_lines[index].rvline();
            let vline = self.visual_lines[index].vline();
            let interval = self.visual_lines[index].origin_interval;
            // todo?
            let origin_line = self.visual_lines[index].origin_folded_line;
            vline_infos.push(VLineInfo {
                interval,
                rvline,
                origin_line,
                vline,
            });
        }
        vline_infos
    }
}
