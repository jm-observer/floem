use std::ops::Range;

use crate::{
    peniko::Color,
    text::{Attrs, AttrsList},
};
use floem_editor_core::cursor::CursorAffinity;
use smallvec::SmallVec;
use tracing::info;

/// `PhantomText` is for text that is not in the actual document, but should be rendered with it.
///
/// Ex: Inlay hints, IME text, error lens' diagnostics, etc
#[derive(Debug, Clone, Default)]
pub struct PhantomText {
    /// The kind is currently used for sorting the phantom text on a line
    pub kind: PhantomTextKind,
    /// Column on the line that the phantom text should be displayed at
    ///
    /// 在原始文本的行
    pub line: usize,
    /// Column on the line that the phantom text should be displayed at.Provided by lsp
    ///
    /// 在原始行文本的位置
    pub col: usize,
    /// Column on the line that the phantom text should be displayed at.Provided by lsp
    ///
    /// 合并后原始行文本的位置
    pub merge_col: usize,
    /// Provided by calculate.Column index in final line.
    ///
    /// 在最终行文本的位置
    pub final_col: usize,
    /// the affinity of cursor, e.g. for completion phantom text,
    /// we want the cursor always before the phantom text
    pub affinity: Option<CursorAffinity>,
    pub text: String,
    pub font_size: Option<usize>,
    // font_family: Option<FontFamily>,
    pub fg: Option<Color>,
    pub bg: Option<Color>,
    pub under_line: Option<Color>,
}

impl PhantomText {

    pub fn next_line(&self) -> Option<usize> {
        if let PhantomTextKind::LineFoldedRang {next_line, ..} = self.kind {
            next_line
        } else {
            None
        }
    }

    /// [start..end]
    pub fn final_col_range(&self) -> Option<(usize, usize)> {
        if self.text.is_empty() {
            None
        } else {
            Some((self.final_col, self.final_col + self.text.len() - 1))
        }
    }

    pub fn next_origin_col(&self) -> usize {
        if let PhantomTextKind::LineFoldedRang {len, ..} = self.kind {
            self.col + len
        } else {
            self.col
        }
    }

    pub fn log(&self) {
        tracing::info!("{:?} line={} col={} final_col={} text={} text.len()={}", self.kind, self.line, self.merge_col, self.final_col, self.text, self.text.len());
    }
}

#[derive(Debug, Clone, Copy, Ord, Eq, PartialEq, PartialOrd, Default)]
pub enum PhantomTextKind {
    #[default]
    /// Input methods
    Ime,
    Placeholder,
    /// Completion lens / Inline completion
    Completion,
    /// Inlay hints supplied by an LSP/PSP (like type annotations)
    InlayHint,
    /// Error lens
    Diagnostic,
    // 行内折叠。跨行折叠也都转换成行内折叠
    LineFoldedRang {
        next_line: Option<usize>,
        len: usize,
    },
    // 折叠的起始位置。折叠是否跨行、结束位置的行、列位置
    // CrossLineFoldedRangStart {
    //     // 用于是否衔接下一行
    //     end_line: usize,
    //     // 行末字符
    //     end_character: usize,
    // },
    // 折叠的结束位置。结束位置的行
    // CrossLineFoldedRangEnd {
    //     end_line: usize,
    //     start_character: usize,
    // },
}

/// Information about the phantom text on a specific line.  
///
/// This has various utility functions for transforming a coordinate (typically a column) into the
/// resulting coordinate after the phantom text is combined with the line's real content.
#[derive(Debug, Default, Clone)]
pub struct PhantomTextLine {
    visual_line: usize,
    // 原文本的长度，包括换行符等，原始单行
    origin_text_len: usize,
    // 最后展现的长度，包括幽灵文本、换行符.
    final_text_len: usize,
    /// This uses a smallvec because most lines rarely have more than a couple phantom texts
    text: SmallVec<[PhantomText; 6]>,
}

impl PhantomTextLine {
    pub fn new(line: usize,
               origin_text_len: usize, mut text: SmallVec<[PhantomText; 6]>) -> Self {
        text.sort_by(|a, b| {
            if a.merge_col == b.merge_col {
                a.kind.cmp(&b.kind)
            } else {
                a.merge_col.cmp(&b.merge_col)
            }
        });
        let line = Self {
            final_text_len: origin_text_len,
            visual_line: line + 1,
            origin_text_len, text
        };
        line.update()
    }

    pub fn log(&self) {
        tracing::info!("PhantomTextLine visual_line={} origin_text_len={}", self.visual_line, self.origin_text_len);
        for phantom in &self.text {
            phantom.log();
        }
        tracing::info!("");
    }

    /// 因为折叠的原因，所以重新计算final_col。当前数据只有本行！！！
    fn update(mut self) -> Self {
        let mut offset = 0i32;
        for phantom in &mut self.text {
            match phantom.kind {
                PhantomTextKind::LineFoldedRang{ len, .. } => {
                    phantom.final_col = usize_offset(phantom.merge_col, offset);
                    offset = offset + phantom.text.len() as i32 - len as i32;
                }
                _ => {
                    phantom.final_col = usize_offset(phantom.merge_col, offset);
                    offset += phantom.text.len() as i32;
                }
            }
        }
        self.final_text_len = usize_offset(self.origin_text_len, offset);
        self
    }

    // pub fn add_phantom_style(
    //     &self,
    //     attrs_list: &mut AttrsList,
    //     attrs: Attrs,
    //     font_size: usize,
    //     phantom_color: Color,
    //     collapsed_line_col: usize,
    // ) {
    //     // Apply phantom text specific styling
    //     for (_offset, size, _, (_line, final_col), phantom) in self.offset_size_iter_2() {
    //         if phantom.text.is_empty() {
    //             continue
    //         }
    //         let mut attrs = attrs;
    //         if let Some(fg) = phantom.fg {
    //             attrs = attrs.color(fg);
    //         } else {
    //             attrs = attrs.color(phantom_color)
    //         }
    //         if let Some(phantom_font_size) = phantom.font_size {
    //             attrs = attrs.font_size(phantom_font_size.min(font_size) as f32);
    //         }
    //
    //         let start = final_col + collapsed_line_col;
    //         let end = final_col + size + collapsed_line_col;
    //         // tracing::info!("{} {start}-{end} final_col={final_col} collapsed_line_col={collapsed_line_col} {}", self.visual_line, phantom.text);
    //         attrs_list.add_span(
    //             start..end,
    //             attrs,
    //         );
    //     }
    // }

    /// 被折叠的范围。用于计算因折叠导致的原始文本的样式变化
    ///
    // pub fn floded_ranges(&self) -> Vec<Range<usize>> {
    //     let mut ranges = Vec::new();
    //     for item in &self.text {
    //         if let PhantomTextKind::FoldedRangStart { same_line, end_character, .. } = item.kind {
    //             if same_line {
    //                 ranges.push(item.final_col..item.col);
    //             } else {
    //                 ranges.push(item.final_col..end_character);
    //             }
    //         } else if let PhantomTextKind::CrossLineFoldedRangEnd{..} = item.kind {
    //             ranges.push(0..item.final_col);
    //         }
    //     }
    //     ranges
    // }

    // /// Translate a column position into the text into what it would be after combining
    // /// 求原始文本在最终文本的位置。场景：计算原始文本的样式在最终文本的位置。
    // ///
    // /// 最终文本的位置 = 原始文本位置 + 之前的幽灵文本长度
    // ///
    // /// todo remove Option
    // pub fn col_at(&self, pre_col: usize) -> Option<usize> {
    //     // if self.visual_line == 11 {
    //     //     tracing::info!("11");
    //     // }
    //     let mut last_offset = 0;
    //     for (_col_shift, _size, (_line, col), (__final_line, final_col), _phantom) in self.offset_size_iter_2() {
    //         // if self.visual_line == 11 {
    //         //     tracing::info!("{pre_col} {col_shift} {size} {_line} {col} {line} {final_col} {:?}", _phantom);
    //         // }
    //         if pre_col <= col {
    //             break;
    //         }
    //         last_offset = final_col as i32 - col as i32;
    //     }
    //     // if self.visual_line == 11 {
    //     //     tracing::info!("\n");
    //     // }
    //     let final_pre_col = pre_col as i32 + last_offset;
    //     if final_pre_col < 0 {
    //         None
    //     } else {
    //         Some(final_pre_col as usize)
    //     }
    // }

    // /// Translate a column position into the text into what it would be after combining
    // ///
    // /// 将列位置转换为合并后的文本位置
    // ///
    // /// If `before_cursor` is false and the cursor is right at the start then it will stay there
    // /// (Think 'is the phantom text before the cursor')
    // pub fn col_after(&self, pre_col: usize, before_cursor: bool) -> usize {
    //     let mut last = pre_col;
    //     for (col_shift, size, (_line, col), text) in self.offset_size_iter() {
    //         // if col_shift < 0 {
    //         //     tracing::warn!("offset < 0 {:?}", text);
    //         //     // continue;
    //         // }
    //         // if size < 0 {
    //         //     // tracing::debug!("size < 0 {:?}", text.kind);
    //         //     assert_eq!(text.kind, PhantomTextKind::CrossLineFoldedRangEnd);
    //         //     // continue;
    //         // }
    //
    //         let before_cursor = match text.affinity {
    //             Some(CursorAffinity::Forward) => true,
    //             Some(CursorAffinity::Backward) => false,
    //             None => before_cursor,
    //         };
    //
    //         if pre_col > col || (pre_col == col && before_cursor) {
    //             last = pre_col + col_shift + size;
    //         }
    //     }
    //
    //     last
    // }

    /// Translate a column position into the position it would be before combining
    ///
    /// 将列位置转换为合并前的位置，也就是原始文本的位置？意义在于计算光标的位置（光标是用原始文本的offset来记录位置的）
    ///
    /// return (line, index)
    // pub fn before_col_2(&self, col: usize, _tracing: bool) -> (usize, usize) {
    //
    //     // if tracing {
    //     //     tracing::info!("col={col} {}", self.final_text_len);
    //     //     self.log("before_col_2");
    //     // }
    //
    //     let col = col.min(self.final_text_len);
    //     let mut last_line = self.visual_line - 1;
    //     let mut last_final_col = 0;
    //     let mut last_origin_col = 0;
    //     // (最终文本上该幽灵文本前其他幽灵文本的总长度，幽灵文本的长度，幽灵文本在原始文本的字符位置，幽灵文本)
    //     for (_col_shift, size, (origin_line, origin_col), (_final_line, final_col), _phantom) in self.offset_size_iter_2() {
    //         let shifted_start = final_col;
    //         let shifted_end = final_col + size;
    //         if col >= shifted_end {
    //             // continue to judge next phantom
    //             last_line = origin_line;
    //             last_final_col = final_col + size;
    //             last_origin_col = origin_col;
    //         // } else if col == shifted_end {
    //         //     // shifted_end is the cursor should be locate.
    //         //     last_col_shift = col_shift + size;
    //         } else if shifted_start < col && col < shifted_end {
    //             return (origin_line, origin_col);
    //         } else if col < shifted_start {
    //             return (last_line, col - last_final_col + last_origin_col);
    //         }
    //
    //     }
    //     (last_line, col - last_final_col + last_origin_col)
    // }

    // /// Insert the hints at their positions in the text
    // /// Option<(collapsed line, collapsed col index)>
    // pub fn combine_with_text<'a>(&self, text: &'a str) -> (Cow<'a, str>, Option<usize>) {
    //     let mut text = Cow::Borrowed(text);
    //     let mut col_shift: i32 = 0;
    //     let mut next_line_info = None;
    //
    //     for phantom in self.text.iter() {
    //         let location = phantom.col as i32 + col_shift;
    //         if location < 0 {
    //             tracing::error!("{:?} {}", phantom.kind, phantom.text);
    //             continue;
    //         }
    //         let location = location as usize;
    //         // Stop iterating if the location is bad
    //         if text.get(location..).is_none() {
    //             return (text, None);
    //         }
    //
    //         let mut text_o = text.into_owned();
    //
    //         if let PhantomTextKind::LineFoldedRang {
    //             next_line, len,
    //         } = phantom.kind
    //         {
    //             next_line_info = next_line;
    //                 let mut new_text_o = text_o.subseq(Interval::new(0, location));
    //                 new_text_o.push_str(&phantom.text);
    //                 // new_text_o.push_str(
    //                 //     &text_o.subseq(Interval::new(end_character, text_o.len())),
    //                 // );
    //                 col_shift = col_shift + phantom.text.len() as i32
    //                     - len as i32;
    //         } else {
    //             text_o.insert_str(location, &phantom.text);
    //             col_shift += phantom.text.len() as i32;
    //         }
    //
    //         text = Cow::Owned(text_o);
    //     }
    //
    //     (text, next_line_info)
    // }

    pub fn folded_line(&self) -> Option<usize> {
        if let Some(text) = self.text.iter().last() {
            if let PhantomTextKind::LineFoldedRang {next_line, ..} = text.kind {
                return next_line;
            }
        }
        None
    }

    // /// Iterator over (col_shift, size, hint, pre_column)
    // /// Note that this only iterates over the ordered text, since those depend on the text for where
    // /// they'll be positioned
    // /// (finally col index, phantom len, phantom at origin text index, phantom)
    // ///
    // /// (最终文本上该幽灵文本前其他幽灵文本的总长度，幽灵文本的长度，(幽灵文本在原始文本的字符位置), (幽灵文本在最终文本的字符位置)，幽灵文本)
    // ///
    // /// 所以原始文本在最终文本的位置= 原始位置 + 之前的幽灵文本总长度
    // ///
    // pub fn offset_size_iter(&self) -> impl Iterator<Item = (usize, usize, (usize, usize), &PhantomText)> + '_ {
    //     let mut col_shift = 0usize;
    //     let mut line = self.visual_line - 1;
    //     self.text.iter().map(move |phantom| {
    //         let rs = match phantom.kind {
    //             PhantomTextKind::FoldedRangStart {
    //                 same_line,
    //                 end_line, end_character, ..
    //             } => {
    //                 let pre_col_shift = col_shift;
    //                 let phantom_line = line;
    //                 if same_line {
    //                     col_shift = col_shift + phantom.text.len()
    //                         - (end_character - phantom.col) ;
    //                 } else {
    //                     line = end_line;
    //                     col_shift += phantom.text.len();
    //                 }
    //                 (
    //                     pre_col_shift,
    //                     phantom.text.len(),
    //                     (phantom_line, phantom.col),
    //                     phantom,
    //                 )
    //             }
    //             PhantomTextKind::CrossLineFoldedRangEnd {..} => {
    //                 // col_shift -= phantom.col;
    //                 (0, 0, (line, phantom.col), phantom)
    //             }
    //             _ => {
    //                 let pre_col_shift = col_shift;
    //                 col_shift += phantom.text.len();
    //                 (
    //                     pre_col_shift,
    //                     phantom.text.len(),
    //                     (line, phantom.col),
    //                     phantom,
    //                 )
    //             }
    //         };
    //         tracing::debug!(
    //             "visual_line={} offset={} len={} col={:?} text={} {:?}",
    //             self.visual_line,
    //             rs.0,
    //             rs.1,
    //             rs.2,
    //             rs.3.text,
    //             rs.3.kind
    //         );
    //         rs
    //     })
    // }

    // /// (最终文本上该幽灵文本前其他幽灵文本的总长度，幽灵文本的长度，(幽灵文本在原始文本的字符位置), (幽灵文本在最终文本的字符位置)，幽灵文本)
    // ///
    // /// 所以原始文本在最终文本的位置= 原始位置 + 之前的幽灵文本总长度
    // ///
    // pub fn offset_size_iter_2(&self) -> impl Iterator<Item = (usize, usize, (usize, usize), (usize, usize), &PhantomText)> + '_ {
    //     let mut before_phantom_len = 0usize;
    //     let line = self.visual_line - 1;
    //     self.text.iter().map(move |phantom| {
    //         match phantom.kind {
    //             PhantomTextKind::LineFoldedRang{..} => {
    //                 (before_phantom_len, 0, (phantom.line, phantom.col), (line, phantom.final_col), phantom)
    //             }
    //             _ => {
    //                 let current_before_phantom_len = before_phantom_len;
    //                 before_phantom_len += phantom.text.len();
    //                 (
    //                     current_before_phantom_len,
    //                     phantom.text.len(),
    //                     (phantom.line, phantom.col), (line, phantom.final_col),
    //                     phantom,
    //                 )
    //             }
    //         }
    //     })
    // }

    // pub fn apply_attr_styles(&self, default: Attrs, attrs_list: &mut AttrsList) {
    //     for (offset, size, (_line, col), phantom) in self.offset_size_iter() {
    //         let start = col + offset;
    //         let end = start + size;
    //
    //         let mut attrs = default;
    //         if let Some(fg) = phantom.fg {
    //             attrs = attrs.color(fg);
    //         }
    //         if let Some(phantom_font_size) = phantom.font_size {
    //             attrs = attrs.font_size((phantom_font_size as f32).min(attrs.font_size));
    //         }
    //
    //         attrs_list.add_span(start..end, attrs);
    //     }
    // }
}


#[derive(Debug, Default, Clone)]
pub struct PhantomTextMultiLine {
    pub visual_line: usize,
    // 原文本的长度，包括换行符等，单行？包括合并行？所有合并在该行的原始行的总长度
    pub origin_text_len: usize,
    // 最后展现的长度，包括幽灵文本、换行符、包括后续的折叠行
    final_text_len: usize,
    /// This uses a smallvec because most lines rarely have more than a couple phantom texts
    pub text: SmallVec<[PhantomText; 6]>,
    pub lines: Vec<PhantomTextLine>,
}

impl PhantomTextMultiLine {
    pub fn new(line: PhantomTextLine) -> Self {
        let mut text = line.text.clone();
        text.sort_by(|a, b| {
            if a.merge_col == b.merge_col {
                a.kind.cmp(&b.kind)
            } else {
                a.merge_col.cmp(&b.merge_col)
            }
        });
        Self {
            visual_line: line.visual_line,
            origin_text_len: line.origin_text_len, final_text_len: line.final_text_len, text,
            lines: vec![line],
        }
    }

    pub fn merge(&mut self, line: PhantomTextLine) {
        let origin_text_len = self.origin_text_len;
        self.origin_text_len += line.origin_text_len;
        let final_text_len = self.final_text_len;
        self.final_text_len += line.final_text_len;
        for mut phantom in line.text.clone() {
            phantom.merge_col += origin_text_len;
            phantom.final_col += final_text_len;
            self.text.push(phantom);
        }
        self.lines.push(line);
    }

    pub fn final_text_len(&self) -> usize {
        self.final_text_len
    }
    pub fn log(&self, _msg: &str) {
        tracing::info!("{_msg} visual_line={} origin_text_len={} final_text_len={}", self.visual_line, self.origin_text_len, self.final_text_len);
        for phantom in &self.text {
            phantom.log();
        }
        tracing::info!("");
    }

    pub fn update_final_text_len(&mut self, _len: usize) {
        self.final_text_len = _len;
    }
    pub fn add_phantom_style(
        &self,
        attrs_list: &mut AttrsList,
        attrs: Attrs,
        font_size: usize,
        phantom_color: Color,
    ) {
        if !self.text.is_empty() {
            tracing::info!("add_phantom_style start visual_line={}", self.visual_line);
        }
        for phantom in &self.text {
            tracing::info!("{phantom:?}");
            if phantom.text.is_empty() {
                continue
            }
            let mut attrs = attrs;
            if let Some(fg) = phantom.fg {
                attrs = attrs.color(fg);
            } else {
                attrs = attrs.color(phantom_color)
            }
            if let Some(phantom_font_size) = phantom.font_size {
                attrs = attrs.font_size(phantom_font_size.min(font_size) as f32);
            }
            attrs_list.add_span(
                phantom.final_col..(phantom.final_col + phantom.text.len()),
                attrs,
            );
        }
    }

    /// 被折叠的范围。用于计算因折叠导致的原始文本的样式变化
    ///
    pub fn floded_ranges(&self) -> Vec<Range<usize>> {
        let mut ranges = Vec::new();
        for item in &self.text {
            if let PhantomTextKind::LineFoldedRang { .. } = item.kind {
                    ranges.push(item.final_col..self.final_text_len);
            }
        }
        ranges
    }

    /// Translate a column position into the text into what it would be after combining
    /// 求原始文本在最终文本的位置。场景：计算原始文本的样式在最终文本的位置。
    ///
    /// 最终文本的位置 = 原始文本位置 + 之前的幽灵文本长度
    ///
    pub fn col_at(&self, pre_col: usize) -> Option<usize> {
        if pre_col >= self.origin_text_len {
            return None;
        }
        // "0123456789012345678901234567890123456789
        // "    if true {nr    } else {nr    }nr"
        // "    if true {...} else {...}nr"
        // "0123456789012345678901234567890123456789
        //              s    e     s    e
        for text in &self.text {
            let (col_start, col_end)  = if let PhantomTextKind::LineFoldedRang {
                len, ..
            } = &text.kind {
                (text.merge_col, text.merge_col + *len)
            } else {
                (text.merge_col, text.merge_col + text.text.len())
            };
            if pre_col < col_start {
                return Some(text.final_col - (text.merge_col - pre_col));
            } else if pre_col >= col_start && pre_col < col_end {
                return None
            }
        }
        Some(self.final_text_len - (self.origin_text_len - pre_col))
    }

    /// Translate a column position into the text into what it would be after combining
    ///
    /// 将列位置转换为合并后的文本位置
    ///
    /// If `before_cursor` is false and the cursor is right at the start then it will stay there
    /// (Think 'is the phantom text before the cursor')
    pub fn col_after(&self, pre_col: usize, before_cursor: bool) -> usize {
        let mut last = pre_col;
        for (col_shift, size, (_line, col), text) in self.offset_size_iter() {
            // if col_shift < 0 {
            //     tracing::warn!("offset < 0 {:?}", text);
            //     // continue;
            // }
            // if size < 0 {
            //     // tracing::debug!("size < 0 {:?}", text.kind);
            //     assert_eq!(text.kind, PhantomTextKind::CrossLineFoldedRangEnd);
            //     // continue;
            // }

            let before_cursor = match text.affinity {
                Some(CursorAffinity::Forward) => true,
                Some(CursorAffinity::Backward) => false,
                None => before_cursor,
            };

            if pre_col > col || (pre_col == col && before_cursor) {
                last = pre_col + col_shift + size;
            }
        }

        last
    }

    /// Translate a column position into the text into what it would be after combining
    ///
    /// it only takes `before_cursor` in the params without considering the
    /// cursor affinity in phantom text
    pub fn col_after_force(&self, pre_col: usize, before_cursor: bool) -> usize {
        let mut last = pre_col;
        for (col_shift, size, (_line, col), _text) in self.offset_size_iter() {
            // if col_shift < 0 {
            //     tracing::warn!("offset < 0 {:?}", text);
            //     // continue;
            // }
            // if size < 0 {
            //     tracing::debug!("size < 0 {:?}", text.kind);
            //     assert_eq!(text.kind, PhantomTextKind::CrossLineFoldedRangEnd);
            //     // continue;
            // }
            if pre_col > col || (pre_col == col && before_cursor) {
                last = pre_col + col_shift + size;
            }
        }

        last
    }

    /// Translate a column position into the text into what it would be after combining

    /// If `before_cursor` is false and the cursor is right at the start then it will stay there
    ///
    /// (Think 'is the phantom text before the cursor')
    ///
    /// This accepts a `PhantomTextKind` to ignore. Primarily for IME due to it needing to put the
    /// cursor in the middle.
    pub fn col_after_ignore(
        &self,
        pre_col: usize,
        before_cursor: bool,
        skip: impl Fn(&PhantomText) -> bool,
    ) -> usize {
        let mut last = pre_col;
        for (col_shift, size, (_line, col), phantom) in self.offset_size_iter() {
            if skip(phantom) {
                continue;
            }

            if pre_col > col || (pre_col == col && before_cursor) {
                last = pre_col + col_shift + size;
            }
        }

        last
    }

    /// Translate a column position into the position it would be before combining
    ///
    /// 将列位置转换为合并前的位置，也就是原始文本的位置？意义在于计算光标的位置（光标是用原始文本的offset来记录位置的）
    ///
    /// return (line, index)
    pub fn before_col(&self, col: usize) -> usize {
        let mut last = col;
        let mut _line = self.visual_line - 1;
        // (最终文本上该幽灵文本前其他幽灵文本的总长度，幽灵文本的长度，幽灵文本在原始文本的字符位置，幽灵文本)
        for (col_shift, size, (_, hint_col), _phantom) in self.offset_size_iter() {
            // if let PhantomTextKind::CrossLineFoldedRangStart { end_line, .. } = &phantom.kind {
            //         _line = *end_line;
            // }
            // if self.visual_line == 10 {
            //     tracing::info!("col_shift={col_shift} size={size} hint_col={hint_col} {phantom:?}");
            // }
            // if col_shift < 0 {
            //     col_shift = 0;
            // }
            let shifted_start = hint_col + col_shift;
            let shifted_end = hint_col + col_shift + size;

            if col >= shifted_start {
                if col >= shifted_end {
                    last = col - col_shift - size;
                } else {
                    last = hint_col;
                    break;
                }
            } else {
                break;
            }
        }
        last
    }

    /// Translate a column position into the position it would be before combining
    ///
    /// 将列位置转换为合并前的位置，也就是原始文本的位置？意义在于计算光标的位置（光标是用原始文本的offset来记录位置的）
    ///
    /// return (line, index)
    pub fn origin_position_of_final_col(&self, col: usize) -> (usize, usize) {
        if self.text.is_empty() {
            return (self.visual_line - 1, self.origin_text_len.min(col));
        };
        let mut text_iter = self.text.iter();
        let mut line = self.visual_line - 1;
        let mut final_start = 0;
        let mut origin_start = 0;
        let col = col.min(self.final_text_len - 1);
        for text in text_iter {
            let Some((phantom_final_start, phantom_final_end)) = text.final_col_range() else {
                if col < text.final_col {
                    return (line, origin_start + (col - final_start));
                }
                origin_start = text.next_origin_col();
                final_start = text.final_col;
                continue;
            };
            //  [origin_start                     [text.col
            //  [final_start       ..col..        [phantom_final_start   ..col..  phantom_final_end]  ..col..
            if col < phantom_final_start {
                return (line, origin_start + (col - final_start));
            } else if phantom_final_start <= col && col <= phantom_final_end {
                return (line, text.col - 1)
            }

            origin_start = text.next_origin_col();
            final_start = phantom_final_end + 1;
            if let Some(next_line) = text.next_line() {
                line = next_line;
                origin_start = 0;
            }
        }
        //  [last_origin_end      ..col..        [text.col
        //  [last_final_end       ..col..        final_text_len)
        let new_col = col.min(self.final_text_len - 1);
        if new_col < final_start {
            self.log("overflow");
            info!("col={col} final_start={final_start} origin_start={origin_start} line={line}");
        }
        (line, origin_start + (new_col - final_start))
    }

    // /// Insert the hints at their positions in the text
    // /// Option<(collapsed line, collapsed col index)>
    // pub fn combine_with_text<'a>(&self, text: &'a str) -> (Cow<'a, str>, Option<(usize, usize)>) {
    //     let mut text = Cow::Borrowed(text);
    //     let mut col_shift: i32 = 0;
    //
    //     for phantom in self.text.iter() {
    //         let location = phantom.col as i32 + col_shift;
    //         if location < 0 {
    //             tracing::error!("{:?} {}", phantom.kind, phantom.text);
    //             continue;
    //         }
    //         let location = location as usize;
    //         // Stop iterating if the location is bad
    //         if text.get(location..).is_none() {
    //             return (text, None);
    //         }
    //
    //         let mut text_o = text.into_owned();
    //
    //         if let PhantomTextKind::LineFoldedRang {
    //             len, ..
    //         } = phantom.kind
    //         {
    //                 let mut new_text_o = text_o.subseq(Interval::new(0, location));
    //                 new_text_o.push_str(&phantom.text);
    //                 // new_text_o.push_str(
    //                 //     &text_o.subseq(Interval::new(end_character, text_o.len())),
    //                 // );
    //                 col_shift = col_shift + phantom.text.len() as i32
    //                     - len as i32;
    //         } else {
    //             text_o.insert_str(location, &phantom.text);
    //             col_shift += phantom.text.len() as i32;
    //         }
    //
    //         text = Cow::Owned(text_o);
    //     }
    //
    //     (text, None)
    // }

    /// Iterator over (col_shift, size, hint, pre_column)
    /// Note that this only iterates over the ordered text, since those depend on the text for where
    /// they'll be positioned
    /// (finally col index, phantom len, phantom at origin text index, phantom)
    ///
    /// (最终文本上该幽灵文本前其他幽灵文本的总长度，幽灵文本的长度，(幽灵文本在原始文本的字符位置), (幽灵文本在最终文本的字符位置)，幽灵文本)
    ///
    /// 所以原始文本在最终文本的位置= 原始位置 + 之前的幽灵文本总长度
    ///
    pub fn offset_size_iter(&self) -> impl Iterator<Item = (usize, usize, (usize, usize), &PhantomText)> + '_ {
        let mut col_shift = 10usize;
        let line = self.visual_line - 1;
        self.text.iter().map(move |phantom| {
            let rs = match phantom.kind {
                PhantomTextKind::LineFoldedRang {
                    ..
                } => {
                    let pre_col_shift = col_shift;
                    let phantom_line = line;
                        col_shift += phantom.text.len();
                    (
                        pre_col_shift,
                        phantom.text.len(),
                        (phantom_line, phantom.merge_col),
                        phantom,
                    )
                }
                // PhantomTextKind::CrossLineFoldedRangStart {
                //     end_line,  ..
                // } => {
                //     let pre_col_shift = col_shift;
                //     let phantom_line = line;
                //         line = end_line;
                //         col_shift += phantom.text.len();
                //     (
                //         pre_col_shift,
                //         phantom.text.len(),
                //         (phantom_line, phantom.col),
                //         phantom,
                //     )
                // }
                // PhantomTextKind::CrossLineFoldedRangEnd {..} => {
                //     // col_shift -= phantom.col;
                //     (0, 0, (line, phantom.col), phantom)
                // }
                _ => {
                    let pre_col_shift = col_shift;
                    col_shift += phantom.text.len();
                    (
                        pre_col_shift,
                        phantom.text.len(),
                        (line, phantom.merge_col),
                        phantom,
                    )
                }
            };
            tracing::debug!(
                "visual_line={} offset={} len={} col={:?} text={} {:?}",
                self.visual_line,
                rs.0,
                rs.1,
                rs.2,
                rs.3.text,
                rs.3.kind
            );
            rs
        })
    }

    pub fn apply_attr_styles(&self, default: Attrs, attrs_list: &mut AttrsList) {
        for (offset, size, (_line, col), phantom) in self.offset_size_iter() {
            let start = col + offset;
            let end = start + size;

            let mut attrs = default;
            if let Some(fg) = phantom.fg {
                attrs = attrs.color(fg);
            }
            if let Some(phantom_font_size) = phantom.font_size {
                attrs = attrs.font_size((phantom_font_size as f32).min(attrs.font_size));
            }

            attrs_list.add_span(start..end, attrs);
        }
    }

    pub fn final_line_content(&self, origin: &str) -> String {
        let rs = combine_with_text(&self.text, self.origin_text_len, origin);
        if !self.text.is_empty() {
            info!("visual_line={} {origin} {rs}", self.visual_line);
        }
        rs
    }
}

fn combine_with_text(lines: &SmallVec<[PhantomText; 6]>, origin_text_len: usize, origin: &str) -> String {
    let mut rs = String::new();
    let mut latest_col = 0;
    for text in lines {
        rs.push_str(sub_str(origin, latest_col, text.merge_col));
        rs.push_str(text.text.as_str());
        if let PhantomTextKind::LineFoldedRang { len, .. } = text.kind {
            latest_col = text.merge_col + len;
        } else {
            latest_col = text.merge_col;
        }
    }
    rs.push_str(sub_str(origin, latest_col, origin_text_len));
    rs
}

fn usize_offset(val: usize, offset: i32) -> usize {
    let rs = val as i32 + offset;
    assert!(rs >= 0);
    rs as usize
}


/// Not allowed to cross the range??
pub struct Ranges {
    ranges: Vec<Range<usize>>
}

impl Ranges {
    pub fn except(&self, mut rang: Range<usize>) -> Vec<Range<usize>> {
        let mut final_ranges = Vec::new();
        for exc in &self.ranges {
            if exc.end <= rang.start {
                // no change
            } else if exc.start <= rang.start && rang.start < exc.end && exc.end < rang.end {
                rang.start = exc.end
            } else if rang.start < exc.start && exc.end <= rang.end {
                final_ranges.push(rang.start..exc.start);
                rang.start = exc.end;
            } else if rang.start < exc.start && rang.end >= exc.start && rang.end < exc.end {
                rang.end = exc.start;
                break;
            } else if rang.end <= exc.start {
                break;
            } else if exc.start <= rang.start && rang.end <= exc.end {
                return Vec::with_capacity(0);
            } else {
                tracing::warn!("{exc:?} {rang:?}");
            }
        }
        if rang.start < rang.end {
            final_ranges.push(rang);
        }
        final_ranges
    }
}


fn sub_str(text: &str, begin: usize, end: usize) -> &str {
    unsafe {
        text.get_unchecked(begin..end)
    }
}

#[cfg(test)]
mod test {
    #![allow(unused_variables, dead_code)]
    use smallvec::SmallVec;
    use crate::views::editor::phantom_text::{combine_with_text, PhantomText, PhantomTextKind, PhantomTextLine, PhantomTextMultiLine};
    use std::default::Default;

    // "0123456789012345678901234567890123456789
    // "    if true {nr    } else {nr    }nr"
    // "    if true {...} else {...}nr"
    fn init_folded_line(visual_line: usize, folded: bool) -> PhantomTextLine{
        let mut text: SmallVec<[PhantomText; 6]> = SmallVec::new();
        let origin_text_len ;
        match (visual_line, folded) {
            (2, _) => {
                origin_text_len = 15;
                text.push(PhantomText{
                    kind: PhantomTextKind::LineFoldedRang {
                        len: 3, next_line: Some(3)
                    }, line:1, final_col: 12,
                    merge_col: 12, col: 12,
                    text: "{...}".to_string(), ..Default::default()
                });
            }
            (4, false) => {
                origin_text_len = 14;
                text.push(PhantomText{
                    kind: PhantomTextKind::LineFoldedRang {
                        next_line: None,
                        len: 5,
                    },
                    line: 3, final_col: 0, col: 0,
                    merge_col: 0,
                    text: "".to_string(), ..Default::default()
                });
            }
            (4, true) => {
                // "0123456789012345678901234567890123456789
                // "    } else {nr    }nr"
                origin_text_len = 14;
                text.push(PhantomText{
                    kind: PhantomTextKind::LineFoldedRang {
                        next_line: None,
                        len: 5,
                    },
                    line: 3, final_col: 0, col: 0,
                    merge_col: 0,
                    text: "".to_string(), ..Default::default()
                });
                text.push(PhantomText{
                    kind: PhantomTextKind::LineFoldedRang {
                        next_line: Some(5),
                        len: 3,
                    }, line:3, final_col: 11, col: 11,
                    merge_col: 11,
                    text: "{...}".to_string(), ..Default::default()
                });
            }
            (6, _) => {
                origin_text_len = 7;
                text.push(PhantomText{
                    kind: PhantomTextKind::LineFoldedRang {
                        next_line: None,
                        len: 5,
                    },
                    line: 5, final_col: 0, col: 0,
                    merge_col: 0,
                    text: "".to_string(), ..Default::default()
                });
            }
            _ => {
                panic!("");
            }
        }
        PhantomTextLine::new(
            visual_line - 1,
            origin_text_len,
            text,
        )
    }
    // "0         10        20        30
    // "0123456789012345678901234567890123456789
    // "    let a = A;nr
    fn let_data() -> PhantomTextLine {
        let mut text: SmallVec<[PhantomText; 6]> = SmallVec::new();
        let
                origin_text_len = 16;
                text.push(PhantomText{
                    kind: PhantomTextKind::InlayHint,
                    merge_col: 9, line: 6, col: 9,
                    text: ": A ".to_string(), ..Default::default()
                });
        PhantomTextLine::new(
            7,
            origin_text_len,
            text,
        )
    }

    #[test]
    fn test_init() {

        let line2 = init_folded_line(2, false);
        let line4 = init_folded_line(4, false);
        let line_folded_4 = init_folded_line(4, true);
        let line6 = init_folded_line(6, false);
        print_line(&line2);
        check_lines_col(&line2.text, line2.origin_text_len, line2.final_text_len, "    if true {nr", "    if true {...}");
        check_line_final_col(&line2, "    if true {...}");

        print_line(&line4);
        check_lines_col(&line4.text, line4.origin_text_len, line4.final_text_len, "    } else {nr", " else {nr");
        check_line_final_col(&line4, " else {nr");

        print_line(&line_folded_4);
        check_lines_col(&line_folded_4.text, line_folded_4.origin_text_len, line_folded_4.final_text_len, "    } else {nr", " else {...}");
        check_line_final_col(&line_folded_4, " else {...}");

        print_line(&line6);
        check_lines_col(&line6.text, line6.origin_text_len, line6.final_text_len, "    }nr", "nr");
        check_line_final_col(&line6, "nr");

        {
            let let_line = let_data();
            print_line(&let_line);
            let expect_str= "    let a: A  = A;nr";
            check_lines_col(&let_line.text, let_line.origin_text_len, let_line.final_text_len, "    let a = A;nr", expect_str);
            check_line_final_col(&line6, expect_str);
        }
    }

    /**
     **2 |    if a.0 {...} else {...}
     **/
    #[test]
    fn test_merge() {

        let line2 = init_folded_line(2, false);
        let line4 = init_folded_line(4, false);
        let line_folded_4 = init_folded_line(4, true);
        let line6 = init_folded_line(6, false);

        {
            /*
             2 |    if a.0 {...} else {
             */
            let mut lines = PhantomTextMultiLine::new(line2.clone());
            check_lines_col(&lines.text, lines.origin_text_len, lines.final_text_len, "    if true {nr", "    if true {...}");
            lines.merge(line4);
            check_lines_col(&lines.text, lines.origin_text_len, lines.final_text_len, "    if true {nr    } else {nr", "    if true {...} else {nr");
        }
        {
            /*
             2 |    if a.0 {...} else {...}
             */
            let mut lines = PhantomTextMultiLine::new(line2);
            check_lines_col(&lines.text, lines.origin_text_len, lines.final_text_len, "    if true {nr", "    if true {...}");
            // print_lines(&lines);
            // print_line(&line_folded_4);
            lines.merge(line_folded_4);
            // print_lines(&lines);
            check_lines_col(&lines.text, lines.origin_text_len, lines.final_text_len, "    if true {nr    } else {nr", "    if true {...} else {...}");
            lines.merge(line6);
            check_lines_col(&lines.text, lines.origin_text_len, lines.final_text_len, "    if true {nr    } else {nr    }nr", "    if true {...} else {...}nr");
        }
    }

#[test]
    fn check_origin_position_of_final_col() {
    //     check_folded_origin_position_of_final_col();
    // check_let_origin_position_of_final_col();
    check_folded_origin_position_of_final_col_1();
    }
    fn check_let_origin_position_of_final_col() {
        // "0         10        20        30
        // "0123456789012345678901234567890123456789
        // "    let a = A;nr
        // "    let a: A  = A;nr
        // "0123456789012345678901234567890123456789
        // "0         10        20        30
        let let_line = PhantomTextMultiLine::new(let_data());
        print_lines(&let_line);

        let orgin_text: Vec<char> = "    let a: A  = A;nr".chars().into_iter().collect();
        {
            assert_eq!(orgin_text[8], 'a');
            assert_eq!(let_line.origin_position_of_final_col(8).1, 8);
        }
        {
            assert_eq!(orgin_text[11], 'A');
            assert_eq!(let_line.origin_position_of_final_col(11).1, 8);
        }
        {
            assert_eq!(orgin_text[17], ';');
            assert_eq!(let_line.origin_position_of_final_col(17).1, 13);
        }
        {
            assert_eq!(let_line.origin_position_of_final_col(30).1, 15);
        }

    }

    fn check_folded_origin_position_of_final_col_1() {
        //  "0         10        20        30
        //  "0123456789012345678901234567890123456789
        //  "    if true {nr"
        //2 "    } else {nr"
        //  "    if true {...} else {"
        //  "0123456789012345678901234567890123456789
        //  "0         10        20        30
        //              s    e     s    e
        let line = {
            let line2 = init_folded_line(2, false);
            let line_folded_4 = init_folded_line(4, false);
            let mut lines = PhantomTextMultiLine::new(line2);
            lines.merge(line_folded_4);
            lines
        };
        print_lines(&line);
        let orgin_text: Vec<char> = "    if true {...} else {nr".chars().into_iter().collect();
        {
            assert_eq!(orgin_text[9], 'u');
            assert_eq!(line.origin_position_of_final_col(9), (1, 9));
        }
        {
            let index = 12;
            assert_eq!(orgin_text[index], '{');
            assert_eq!(line.origin_position_of_final_col(index), (1, 11));
        }
        {
            let index = 19;
            assert_eq!(orgin_text[index], 'l');
            assert_eq!(line.origin_position_of_final_col(index), (3, 7));
        }
        {
            assert_eq!(line.origin_position_of_final_col(26), (3, 13));
        }
    }
    fn check_folded_origin_position_of_final_col() {
        //  "0         10        20        30
        //  "0123456789012345678901234567890123456789
        //  "    }nr"
        //2 "    } else {nr    }nr"
        //  "    if true {...} else {...}nr"
        //  "0123456789012345678901234567890123456789
        //  "0         10        20        30
        //              s    e     s    e
        let line = get_merged_data();
        print_lines(&line);
        let orgin_text: Vec<char> = "    if true {...} else {...}nr".chars().into_iter().collect();
        {
            assert_eq!(orgin_text[9], 'u');
            assert_eq!(line.origin_position_of_final_col(9), (1, 9));
        }
        {
            let index = 12;
            assert_eq!(orgin_text[index], '{');
            assert_eq!(line.origin_position_of_final_col(index), (1, 11));
        }
        {
            let index = 19;
            assert_eq!(orgin_text[index], 'l');
            assert_eq!(line.origin_position_of_final_col(index), (3, 7));
        }
        {
            let index = 25;
            assert_eq!(orgin_text[index], '.');
            assert_eq!(line.origin_position_of_final_col(index), (3, 10));
        }
        {
            let index = 29;
            assert_eq!(orgin_text[index], 'r');
            assert_eq!(line.origin_position_of_final_col(index), (5, 6));
        }

        {
            let index = 40;
            assert_eq!(line.origin_position_of_final_col(index), (5, 6));
        }

    }

    #[test]
    fn check_col_at() {
        // "0         10        20        30
        // "0123456789012345678901234567890123456789
        // "    if true {nr    } else {nr    }nr"
        // "    if true {...} else {...}nr"
        // "0123456789012345678901234567890123456789
        // "0         10        20        30
        //              s    e     s    e
        let line = get_merged_data();
        let orgin_text: Vec<char> = "    if true {nr    } else {nr    }nr".chars().into_iter().collect();
        {
            let index = 35;
            assert_eq!(orgin_text[index], 'r');
            assert_eq!(line.col_at(index), Some(29));
        }
        {
            let index = 26;
            assert_eq!(orgin_text[index], '{');
            assert_eq!(line.col_at(index), None);
        }
        {
            let index = 22;
            assert_eq!(orgin_text[index], 'l');
            assert_eq!(line.col_at(index), Some(19));
        }
        {
            assert_eq!(orgin_text[9], 'u');
            assert_eq!(line.col_at(9), Some(9));
        }
        {
            let index = 12;
            assert_eq!(orgin_text[index], '{');
            assert_eq!(line.col_at(index), None);
        }
        {
            let index = 19;
            assert_eq!(orgin_text[index], '}');
            assert_eq!(line.col_at(index), None);
        }
    }

    /*
     2 |    if a.0 {...} else {...}
     */
    fn get_merged_data() -> PhantomTextMultiLine {
        let line2 = init_folded_line(2, false);
        let line_folded_4 = init_folded_line(4, true);
        let line6 = init_folded_line(6, false);
        let mut lines = PhantomTextMultiLine::new(line2);
        lines.merge(line_folded_4);
        lines.merge(line6);
        lines
    }

    fn check_lines_col(lines: &SmallVec<[PhantomText; 6]>, origin_text_len: usize, final_text_len: usize, origin: &str, expect: &str) {
        let rs = combine_with_text(lines, origin_text_len, origin);
        assert_eq!(expect, rs.as_str());
        assert_eq!(final_text_len, expect.len());
    }

    fn check_line_final_col(lines: &PhantomTextLine, rs: &str) {
        for text in &lines.text {
            assert_eq!(text.text.as_str(), sub_str(rs, text.final_col, text.final_col + text.text.len()));
        }
    }

    fn sub_str(text: &str, begin: usize, end: usize) -> &str {
        unsafe {
            text.get_unchecked(begin..end)
        }
    }

    fn print_lines(lines: &PhantomTextMultiLine) {
        println!("visual_line={} origin_text_len={} final_text_len={}", lines.visual_line, lines.origin_text_len, lines.final_text_len);
        for text in &lines.text {
            println!("{:?} line={} col={} merge_col={} final_col={} text={} text.len()={}", text.kind, text.line, text.col, text.merge_col, text.final_col, text.text, text.text.len());
        }
        println!();
    }

    fn print_line(lines: &PhantomTextLine) {
        println!("visual_line={} origin_text_len={} final_text_len={}", lines.visual_line, lines.origin_text_len, lines.final_text_len);
        for text in &lines.text {
            println!("{:?} line={} col={} merge_col={} final_col={} text={} text.len()={}", text.kind, text.line, text.col, text.merge_col, text.final_col, text.text, text.text.len());
        }
        println!();
    }
}
