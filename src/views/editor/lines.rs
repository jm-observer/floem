use std::fmt::{Debug, Formatter};
use std::sync::Arc;
use lapce_xi_rope::Interval;

use floem_editor_core::buffer::rope_text::RopeText;
use floem_editor_core::cursor::CursorAffinity;
use floem_reactive::{Scope, SignalGet};
use tracing::{warn};

use crate::views::editor::{Editor};
use crate::views::editor::layout::TextLayoutLine;
use crate::views::editor::listener::Listener;
use crate::views::editor::visual_line::{LayoutEvent, ResolvedWrap, RVLine, TextLayoutProvider, VLine, VLineInfo};

#[allow(dead_code)]
#[derive(Clone, Copy, Debug)]
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

impl Debug for OriginFoldedLine {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        write!(f, "OriginFoldedLine line_index={} origin_line_start={} origin_line_end={} origin_interval={}",
            self.line_index, self.origin_line_start, self.origin_line_end, self.origin_interval)
    }
}

#[derive(Clone, Debug)]
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

    pub fn vline_info(&self) -> VLineInfo {
        let rvline = self.rvline();
        let vline = self.vline();
        let interval = self.origin_interval;
        // todo?
        let origin_line = self.origin_folded_line;
        VLineInfo {
            interval,
            rvline,
            origin_line,
            vline,
        }
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
    // pub font_sizes: Rc<EditorFontSizes>,
    // font_size_cache_id: FontSizeCacheId,
    wrap: ResolvedWrap,
    pub layout_event: Listener<LayoutEvent>,
    max_width: f64,
    cache_rev: u64,
    // editor: Editor
}

impl Lines {
    pub fn new(cx: Scope) -> Self {
        Self {
            wrap: ResolvedWrap::None,
            // font_size_cache_id: id,
            layout_event: Listener::new_empty(cx),
            origin_lines: vec![],
            origin_folded_lines: vec![],
            visual_lines: vec![],
            max_width: 0.0,
            cache_rev: 0
        }
    }

    // pub fn update_cache_id(&mut self) {
    //     let current_id = self.font_sizes.cache_id();
    //     if current_id != self.font_size_cache_id {
    //         self.font_size_cache_id = current_id;
    //         self.update()
    //     }
    // }

    // pub fn update_font_sizes(&mut self, font_sizes: Rc<EditorFontSizes>) {
    //     self.font_sizes = font_sizes;
    //     self.update()
    // }

    fn clear(&mut self) {
        self.origin_lines.clear();
        self.origin_folded_lines.clear();
        self.visual_lines.clear();
        self.max_width = 0.0
    }

    // return do_update
    pub fn update(&mut self, editor: &Editor) -> bool {
        let doc_rev = editor.doc().cache_rev().get_untracked();
        if doc_rev == self.cache_rev && self.cache_rev != 0 {
            return false;
        }
        self.clear();
        self.cache_rev = doc_rev;
        let rope_text = editor.rope_text();
        let last_line = rope_text
            .last_line();

        let mut current_line = 0;
        let mut origin_folded_line_index = 0;
        let mut visual_line_index = 0;
        while current_line <= last_line {
            let text_layout = editor.new_text_layout(current_line);
            let origin_line_start = text_layout.phantom_text.line;
            let origin_line_end = text_layout.phantom_text.last_line;

            let width = text_layout.text.size().width;
            if width > self.max_width {
                self.max_width = width;
            }

            for origin_line in origin_line_start..=origin_line_end {
                self.origin_lines.push(OriginLine {
                    line_index: origin_line,
                    start_offset: rope_text.offset_of_line(origin_line),
                });
            }

            let mut visual_offset_start = 0;
            let mut visual_offset_end ;
            // [visual_offset_start..visual_offset_end)
            for (origin_folded_line_sub_index, layout) in text_layout.text.line_layout().iter().enumerate() {
                visual_offset_end =  visual_offset_start + layout.glyphs.len();

                let offset_info = text_layout.phantom_text.origin_position_of_final_col(visual_offset_start);
                let origin_interval_start = rope_text.offset_of_line_col(offset_info.0, offset_info.1);

                let offset_info = text_layout.phantom_text.origin_position_of_final_col(visual_offset_end);
                let origin_interval_end = rope_text.offset_of_line_col(offset_info.0, offset_info.1);
                let origin_interval = Interval { start: origin_interval_start, end: origin_interval_end };

                self.visual_lines.push(VisualLine {
                    line_index: visual_line_index,
                    origin_interval,
                    origin_folded_line: origin_folded_line_index,
                    origin_folded_line_sub_index,
                });

                visual_offset_start = visual_offset_end;
                visual_line_index += 1;
            }

            let origin_interval = Interval { start: rope_text.offset_of_line(origin_line_start), end: rope_text.offset_of_line(origin_line_end + 1) };
            self.origin_folded_lines.push(OriginFoldedLine {
                line_index: origin_folded_line_index,
                origin_line_start,
                origin_line_end,
                origin_interval,
                text_layout,
            });

            current_line = origin_line_end + 1;
            origin_folded_line_index += 1;
        }
        // if self.visual_lines.len() > 2 {
        //     tracing::error!("Lines origin_lines={} origin_folded_lines={} visual_lines={}", self.origin_lines.len(), self.origin_folded_lines.len(), self.visual_lines.len());
        //     tracing::error!("{:?}", self.origin_lines);
        //     tracing::error!("{:?}", self.origin_folded_lines);
        //     tracing::error!("{:?}\n", self.visual_lines);
        // }
        warn!("update_lines done");
        true

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
        self.update(editor);
    }

    pub fn max_width(&self) -> f64 {
        self.max_width
    }

    /// ~~视觉~~行的text_layout信息
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
            if folded_line.origin_line_start <= origin_line && origin_line <= folded_line.origin_line_end {
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

    /// 原始字符所在的视觉行，以及行的偏移位置和是否是最后一个字符
    pub fn visual_line_of_offset(&self, origin_line: usize, offset: usize, _affinity: CursorAffinity) -> (VLineInfo, usize, bool) {
        // 位于的原始行，以及在原始行的起始offset
        // let (origin_line, offset_of_line) = self.font_sizes.doc.with_untracked(|x| {
        //     let text = x.text();
        //     let origin_line = text.line_of_offset(offset);
        //     let origin_line_start_offset = text.offset_of_line(origin_line);
        //     (origin_line, origin_line_start_offset)
        // });
        // let mut offset = offset - offset_of_line;
        let folded_line = self.folded_line_of_origin_line(origin_line);
        let mut final_offset  = folded_line.text_layout.phantom_text.final_col_of_col(origin_line, offset, false);
        let folded_line_layout = folded_line.text_layout.text.line_layout();
        let mut sub_line_index = folded_line_layout.len() - 1;
        let mut last_char = false;
        for (index, sub_line) in folded_line_layout.iter().enumerate() {
            if final_offset < sub_line.glyphs.len() {
                sub_line_index = index;
                last_char = final_offset == sub_line.glyphs.len() - 1;
                break;
            } else {
                final_offset -= sub_line.glyphs.len();
            }
        }
        let visual_line = self.visual_line_of_folded_line_and_sub_index(folded_line.line_index, sub_line_index);

        (visual_line.vline_info(), final_offset, last_char)
    }

    pub fn vline_infos(&self, start: usize, end: usize) -> Vec<VLineInfo<VLine>> {
        let start = start.min(self.visual_lines.len() - 1);
        let end = end.min(self.visual_lines.len() - 1);

        let mut vline_infos = Vec::with_capacity(end - start + 1);
        for index in start..=end {
            vline_infos.push(self.visual_lines[index].vline_info());
        }
        vline_infos
    }

    pub fn first_vline_info(&self) -> VLineInfo<VLine> {
        self.visual_lines[0].vline_info()
    }
}
