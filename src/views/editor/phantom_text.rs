#![allow(unused_imports)]
use std::collections::HashMap;
use std::ops::Range;
use lapce_xi_rope::Interval;
use lapce_xi_rope::tree::Leaf;

use crate::{
    peniko::Color,
    text::{Attrs, AttrsList},
};
use floem_editor_core::cursor::CursorAffinity;
use smallvec::SmallVec;
use tracing::{error, info};

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
    /// 在原始行line文本的位置
    pub col: usize,
    /// Column on the line that the phantom text should be displayed at.Provided by lsp
    ///
    /// 合并后，在多行原始行文本（不考虑折叠、幽灵文本）的位置。与col相差前面折叠行的总长度
    pub merge_col: usize,
    /// Provided by calculate.Column index in final line.
    ///
    /// 在最终行文本（考虑折叠、幽灵文本）的位置
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

    pub fn next_final_col(&self) -> usize {
        if let Some((_, end)) = self.final_col_range() {
            end + 1
        } else {
            self.final_col
        }
    }

    pub fn next_origin_col(&self) -> usize {
        if let PhantomTextKind::LineFoldedRang {len, ..} = self.kind {
            self.col + len
        } else {
            self.col
        }
    }

    pub fn next_merge_col(&self) -> usize {
        if let PhantomTextKind::LineFoldedRang {len, ..} = self.kind {
            self.merge_col + len
        } else {
            self.merge_col
        }
    }

    pub fn log(&self) {
        tracing::info!("{:?} line={} col={} final_col={} text={} text.len()={}", self.kind, self.line, self.merge_col, self.final_col, self.text, self.text.len());
    }
}
#[derive(Debug, Clone)]
pub struct OriginText {
    /// 在原始文本的行
    pub line: usize,
    /// Column on the line that the phantom text should be displayed at.Provided by lsp
    ///
    /// 在原始行文本的位置
    pub col: Interval,
    ///
    /// 合并后原始行文本的位置
    pub merge_col: Interval,
    /// Provided by calculate.Column index in final line.
    ///
    /// 在最终行文本的位置
    pub final_col: Interval,
}
#[derive(Debug, Clone)]
pub enum Text {
    Phantom {
        text: PhantomText
    },
    OriginText {
        text: OriginText
    },
    Empty,
}

impl Text {
    fn merge_to(mut self, origin_text_len: usize, final_text_len: usize) -> Self {
        match &mut self {
            Text::Phantom { text } => {
                text.merge_col += origin_text_len;
                text.final_col += final_text_len;
            }
            Text::OriginText { text } => {
                text.merge_col = text.merge_col.translate(origin_text_len);
                text.final_col =  text.final_col.translate(final_text_len);
            }
            _ => {}
        }
        self
    }
}

impl From<PhantomText> for Text {
    fn from(text: PhantomText) -> Self {
        Self::Phantom {text}
    }
}
impl From<OriginText> for Text {
    fn from(text: OriginText) -> Self {
        Self::OriginText {text}
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
        // 被折叠的长度
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
    line: usize,
    // 原文本的长度，包括换行符等，原始单行
    origin_text_len: usize,
    // 最后展现的长度，包括幽灵文本、换行符.
    final_text_len: usize,
    /// This uses a smallvec because most lines rarely have more than a couple phantom texts
    texts: SmallVec<[Text; 6]>,
}

impl PhantomTextLine {
    pub fn new(line: usize,
               origin_text_len: usize, mut phantom_texts: SmallVec<[PhantomText; 6]>) -> Self {
        phantom_texts.sort_by(|a, b| {
            if a.merge_col == b.merge_col {
                a.kind.cmp(&b.kind)
            } else {
                a.merge_col.cmp(&b.merge_col)
            }
        });

        let mut final_last_end = 0;
        let mut origin_last_end = 0;
        let mut merge_last_end = 0;
        let mut texts = SmallVec::new();

        let mut offset = 0i32;
        for mut phantom in phantom_texts {
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
            if final_last_end < phantom.final_col {
                let len = phantom.final_col - final_last_end;
                // insert origin text
                texts.push(OriginText {
                    line: phantom.line,
                    col: Interval::new(origin_last_end, origin_last_end+ len),
                    merge_col: Interval::new(merge_last_end, merge_last_end+ len),
                    final_col: Interval::new(final_last_end, final_last_end+ len),
                }.into());
            }
            final_last_end = phantom.next_final_col();
            origin_last_end = phantom.next_origin_col();
            merge_last_end = phantom.next_merge_col();
            texts.push(phantom.into());
        }

        let len = origin_text_len - origin_last_end;
        if len > 0 {
            texts.push(OriginText {
                line,
                col: Interval::new(origin_last_end, origin_last_end+ len),
                merge_col: Interval::new(merge_last_end, merge_last_end+ len),
                final_col: Interval::new(final_last_end, final_last_end+ len),
            }.into());
        }


        let final_text_len = usize_offset(origin_text_len, offset);
        Self {
            final_text_len,
            line,
            origin_text_len,
            texts
        }

    }


    // 因为折叠的原因，所以重新计算final_col。当前数据只有本行！！！
    // fn update(mut self) -> Self {
    //     let mut offset = 0i32;
    //     for phantom in &mut self.texts {
    //         match phantom.kind {
    //             PhantomTextKind::LineFoldedRang{ len, .. } => {
    //                 phantom.final_col = usize_offset(phantom.merge_col, offset);
    //                 offset = offset + phantom.text.len() as i32 - len as i32;
    //             }
    //             _ => {
    //                 phantom.final_col = usize_offset(phantom.merge_col, offset);
    //                 offset += phantom.text.len() as i32;
    //             }
    //         }
    //     }
    //     self.final_text_len = usize_offset(self.origin_text_len, offset);
    //     self
    // }

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

    // 被折叠的范围。用于计算因折叠导致的原始文本的样式变化
    //
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

    // Translate a column position into the position it would be before combining
    //
    // 将列位置转换为合并前的位置，也就是原始文本的位置？意义在于计算光标的位置（光标是用原始文本的offset来记录位置的）
    //
    // return (line, index)
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
        if let Some(Text::Phantom {text}) = self.texts.iter().last() {
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
    pub line: usize,
    pub last_line: usize,
    // 所有合并在该行的原始行的总长度
    pub origin_text_len: usize,
    // 所有合并在该行的最后展现的长度，包括幽灵文本、换行符、包括后续的折叠行
    pub final_text_len: usize,
    // 各个原始行的行号、原始长度、最后展现的长度
    pub len_of_line: Vec<(usize, usize, usize)>,
    /// This uses a smallvec because most lines rarely have more than a couple phantom texts
    pub text: SmallVec<[Text; 6]>,
    // 可以去掉，仅做记录
    // pub lines: Vec<PhantomTextLine>,
}

impl PhantomTextMultiLine {
    pub fn new(line: PhantomTextLine) -> Self {
        let len_of_line = vec![(line.line, line.origin_text_len, line.final_text_len)];
        Self {
            line: line.line,
            last_line: line.line,
            origin_text_len: line.origin_text_len, final_text_len: line.final_text_len,
            len_of_line,
            text: line.texts,
        }
    }

    pub fn merge(&mut self, line: PhantomTextLine) {
        let index = self.len_of_line.len();
        let last_len = self.len_of_line[index - 1];
        for _ in index..line.line-self.line {
            self.len_of_line.push(last_len);
        }
        self.len_of_line.push((line.line, line.origin_text_len, line.final_text_len));


        let origin_text_len = self.origin_text_len;
        self.origin_text_len += line.origin_text_len;
        let final_text_len = self.final_text_len;
        self.final_text_len += line.final_text_len;
        for phantom in line.texts.clone() {
            self.text.push(phantom.merge_to(origin_text_len, final_text_len));
        }
        self.last_line = line.line;
        // self.lines.push(line);
    }

    pub fn final_text_len(&self) -> usize {
        self.final_text_len
    }

    pub fn update_final_text_len(&mut self, _len: usize) {
        self.final_text_len = _len;
    }

    pub fn iter_phantom_text(&self) -> impl Iterator<Item=&PhantomText> {
        self.text.iter().filter_map(|x| if let Text::Phantom {text} = x {
            Some(text)
        } else {
            None
        })
    }
    pub fn add_phantom_style(
        &self,
        attrs_list: &mut AttrsList,
        attrs: Attrs,
        font_size: usize,
        phantom_color: Color,
    ) {
        self.text.iter().for_each(|x| match x {
            Text::Phantom { text } => {
                if !text.text.is_empty() {
                    let mut attrs = attrs;
                    if let Some(fg) = text.fg {
                        attrs = attrs.color(fg);
                    } else {
                        attrs = attrs.color(phantom_color)
                    }
                    if let Some(phantom_font_size) = text.font_size {
                        attrs = attrs.font_size(phantom_font_size.min(font_size) as f32);
                    }
                    attrs_list.add_span(
                        text.final_col..(text.final_col + text.text.len()),
                        attrs,
                    );
                }
            },
            Text::OriginText { .. } | Text::Empty => {}
        });
    }

    // /// 被折叠的范围。用于计算因折叠导致的原始文本的样式变化
    // ///
    // pub fn floded_ranges(&self) -> Vec<Range<usize>> {
    //     let mut ranges = Vec::new();
    //     for item in &self.text {
    //         if let Text::Phantom {
    //             text
    //         }
    //         if let PhantomTextKind::LineFoldedRang { .. } = item.kind {
    //                 ranges.push(item.final_col..self.final_text_len);
    //         }
    //     }
    //     ranges
    // }

    // /// 最终文本的文本信息
    // fn text_of_final_col(&self, final_col: usize) -> &Text {
    //     self.text_of_final_offset(final_col.min(self.final_text_len - 1)).unwrap()
    // }

    /// 最终行偏移的文本信息。在文本外的偏移返回none
    fn text_of_final_offset(&self, final_offset: usize) -> Option<&Text> {
        self.text.iter().find(|x| {
            match x {
                Text::Phantom { text } => {
                    if text.final_col <= final_offset && final_offset < text.next_final_col() {
                        return true
                    }
                }
                Text::OriginText { text } => {
                    if text.final_col.contains(final_offset) {
                        return true
                    }
                }
                Text::Empty => {
                }
            }
            false
        })
    }

    fn text_of_origin_line_col(&self, origin_line: usize, origin_col: usize) -> Option<&Text> {
        self.text.iter().find(|x| {
            match x {
                Text::Phantom { text } => {
                    if text.line == origin_line && text.col <= origin_col && origin_col < text.next_origin_col() {
                        return true;
                    } else if let Some(next_line) = text.next_line() {
                        if origin_line < next_line {
                            return true;
                        }
                    }
                }
                Text::OriginText { text } => {
                    if text.line == origin_line && text.col.contains(origin_col) {
                        return true
                    }
                }
                Text::Empty => {
                    return true
                }
            }
            false
        })
    }

    fn text_of_merge_col(&self, merge_col: usize) -> Option<&Text> {
        self.text.iter().find(|x| {
            match x {
                Text::Phantom { text } => {
                    if text.merge_col <= merge_col && merge_col <= text.next_merge_col() {
                        return true;
                    }
                }
                Text::OriginText { text } => {
                    if text.merge_col.contains(merge_col) {
                        return true
                    }
                }
                Text::Empty => {
                    return true;
                }
            }
            false
        })
    }

    /// 最终文本的原始文本位移。若为幽灵则返回none.超过最终文本长度，则返回none(不应该在此情况下调用该方法)
    pub fn origin_col_of_final_offset(&self, final_col: usize) -> Option<(usize, usize)> {
        // let final_col = final_col.min(self.final_text_len - 1);
        if let Some(Text::OriginText {
                        text
                    }) = self.text_of_final_offset(final_col) {
                let origin_col = text.col.start + final_col - text.final_col.start;
                return Some((text.line, origin_col));
        }
        None
    }

    /// Translate a column position into the text into what it would be after combining
    /// 求原始文本在最终文本的位置。场景：计算原始文本的样式在最终文本的位置。
    ///
    /// 最终文本的位置 = 原始文本位置 + 之前的幽灵文本长度
    ///
    pub fn col_at(&self, merge_col: usize) -> Option<usize> {
        let text = self.text_of_merge_col(merge_col)?;
        match text {
            Text::Phantom { .. } => {
                None
            }
            Text::OriginText { text} => {
                Some(text.final_col.start + merge_col - text.merge_col.start)
            }
            Text::Empty => {
                None
            }
        }
    }

    // /// Translate a column position into the text into what it would be after combining
    // /// 求原始文本当前行的偏移在在最终文本的位置。场景：计算原始文本的样式在最终文本的位置。
    // ///
    // /// 最终文本的位置 = 原始文本位置 + 之前的幽灵文本长度
    // ///
    // /// 对于折叠行。偏移位置对应的原始文本偏移值。该位置若为幽灵文本，则返回none
    // ///
    // pub fn origin_col_of_final_col(&self, pre_col: usize) -> Option<usize> {
    //     if pre_col >= self.origin_text_len {
    //         return None;
    //     }
    //     // "0123456789012345678901234567890123456789
    //     // "    if true {nr    } else {nr    }nr"
    //     // "    if true {...} else {...}nr"
    //     // "0123456789012345678901234567890123456789
    //     //              s    e     s    e
    //     for text in &self.text {
    //         let (col_start, col_end)  = if let PhantomTextKind::LineFoldedRang {
    //             len, ..
    //         } = &text.kind {
    //             (text.merge_col, text.merge_col + *len)
    //         } else {
    //             (text.merge_col, text.merge_col + text.text.len())
    //         };
    //         if pre_col < col_start {
    //             return Some(text.final_col - (text.merge_col - pre_col));
    //         } else if pre_col >= col_start && pre_col < col_end {
    //             return None
    //         }
    //     }
    //     Some(self.final_text_len - (self.origin_text_len - pre_col))
    // }

    // /// Translate a column position into the text into what it would be after combining
    // ///
    // /// 将列位置转换为合并后的文本位置
    // ///
    // /// If `before_cursor` is false and the cursor is right at the start then it will stay there
    // /// (Think 'is the phantom text before the cursor')
    // pub fn col_after(&self, line: usize, pre_col: usize, before_cursor: bool) -> usize {
    //     self.final_col_of_col(line, pre_col, before_cursor)
    // }
    //
    // /// Translate a column position into the text into what it would be after combining
    // ///
    // /// it only takes `before_cursor` in the params without considering the
    // /// cursor affinity in phantom text
    // pub fn col_after_force(&self, line: usize,  pre_col: usize, before_cursor: bool) -> usize {
    //     self.final_col_of_col(line, pre_col, before_cursor)
    // }

    /// 原始行的偏移字符！！！，的对应的合并后的位置。用于求鼠标的实际位置
    ///
    /// Translate a column position into the text into what it would be after combining
    ///
    /// 暂时不考虑_before_cursor，等足够熟悉了再说
    pub fn final_col_of_col(&self, line: usize, pre_col: usize, _before_cursor: bool) -> usize {
        if self.text.is_empty() {
            return pre_col;
        }
        let text = self.text_of_origin_line_col(line, pre_col);
        if let Some(text) = text {
            match text {
                Text::Phantom { text } => {
                    if text.col == 0 {
                        // 后一个字符
                        text.next_final_col()
                    } else {
                        // 前一个字符
                        text.final_col - 1
                    }
                }
                Text::OriginText { text } => {
                    text.final_col.start + pre_col - text.col.start
                }
                Text::Empty => {
                    panic!()
                }
            }
        } else {
            self.final_text_len - 1
        }
    }

    /// Translate a column position into the position it would be before combining
    ///
    /// 将列位置转换为合并前的位置，也就是原始文本的位置？意义在于计算光标的位置（光标是用原始文本的offset来记录位置的）
    ///
    /// return (line, index)
    pub fn cursor_position_of_final_col(&self, col: usize) -> (usize, usize) {
        let text = self.text_of_final_offset(col);
        if let Some(text) = text {
            match text {
                Text::Phantom { text } => {
                    return (text.line, text.col)
                }
                Text::OriginText { text } => {
                    return (text.line, text.col.start + col - text.final_col.start);
                }
                Text::Empty => {
                    panic!()
                }
            }
        }
        let (line, offset, _) = self.len_of_line.last().unwrap();
        return (*line, (*offset).max(1) -1);
        // let (line, offset, _) = self.len_of_line.last().unwrap();
        // (*line, *offset-1)
    }

    /// Translate a column position into the position it would be before combining
    ///
    /// 获取偏移位置的幽灵文本以及在该幽灵文本的偏移值
    pub fn phantom_text_of_final_col(&self, col: usize) -> Option<(PhantomText, usize)> {
        let text = self.text_of_final_offset(col)?;
        if let Text::Phantom {text} = text{
            Some((text.clone(), text.final_col - col))
        } else {
            None
        }
        // if self.text.is_empty() {
        //     return None;
        // };
        // let text_iter = self.text.iter();
        // for text in text_iter {
        //     let Some((phantom_final_start, phantom_final_end)) = text.final_col_range() else {
        //         continue;
        //     };
        //     //  [origin_start                     [text.col
        //     //  [final_start       ..col..        [phantom_final_start   ..col..  phantom_final_end]  ..col..
        //     if phantom_final_start <= col && col <= phantom_final_end {
        //         return Some((text.clone(), col - phantom_final_start))
        //     }
        // }
        // None
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
    //     let mut col_shift = 10usize;
    //     let line = self.line;
    //     self.text.iter().map(move |phantom| {
    //         let rs = match phantom.kind {
    //             PhantomTextKind::LineFoldedRang {
    //                 ..
    //             } => {
    //                 let pre_col_shift = col_shift;
    //                 let phantom_line = line;
    //                     col_shift += phantom.text.len();
    //                 (
    //                     pre_col_shift,
    //                     phantom.text.len(),
    //                     (phantom_line, phantom.merge_col),
    //                     phantom,
    //                 )
    //             }
    //             _ => {
    //                 let pre_col_shift = col_shift;
    //                 col_shift += phantom.text.len();
    //                 (
    //                     pre_col_shift,
    //                     phantom.text.len(),
    //                     (line, phantom.merge_col),
    //                     phantom,
    //                 )
    //             }
    //         };
    //         tracing::debug!(
    //             "line={} offset={} len={} col={:?} text={} {:?}",
    //             self.line,
    //             rs.0,
    //             rs.1,
    //             rs.2,
    //             rs.3.text,
    //             rs.3.kind
    //         );
    //         rs
    //     })
    // }

    pub fn final_line_content(&self, origin: &str) -> String {
        combine_with_text(&self.text, origin)
    }
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

fn combine_with_text(lines: &SmallVec<[Text; 6]>, origin: &str) -> String {
    let mut rs = String::new();
    // let mut latest_col = 0;
    for text in lines {
        match text {
            Text::Phantom { text } => {
                rs.push_str(text.text.as_str());
            }
            Text::OriginText { text } => {
                rs.push_str(crate::views::editor::phantom_text::sub_str(origin, text.merge_col.start, text.merge_col.end));
            }
            Text::Empty => {
                break;
            }
        }
    }
    rs
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
    use crate::views::editor::phantom_text::{combine_with_text, PhantomText, PhantomTextKind, PhantomTextLine, PhantomTextMultiLine, Text};
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
            6,
            origin_text_len,
            text,
        )
    }

    fn empty_data() -> PhantomTextLine {
        let text: SmallVec<[PhantomText; 6]> = SmallVec::new();
        let
            origin_text_len = 0;
        PhantomTextLine::new(
            6,
            origin_text_len,
            text,
        )
    }
    #[test]
    fn test_all() {
        test_init();
        test_merge();
        check_origin_position_of_final_col();
        check_col_at();
        check_final_col_of_col();
    }

    #[test]
    fn test_init() {

        let line2 = init_folded_line(2, false);
        let line4 = init_folded_line(4, false);
        let line_folded_4 = init_folded_line(4, true);
        let line6 = init_folded_line(6, false);
        print_line(&line2);
        check_lines_col(&line2.texts, line2.final_text_len, "    if true {nr", "    if true {...}");
        check_line_final_col(&line2, "    if true {...}");

        print_line(&line4);
        check_lines_col(&line4.texts, line4.final_text_len, "    } else {nr", " else {nr");
        check_line_final_col(&line4, " else {nr");

        print_line(&line_folded_4);
        check_lines_col(&line_folded_4.texts, line_folded_4.final_text_len,  "    } else {nr", " else {...}");
        check_line_final_col(&line_folded_4, " else {...}");

        print_line(&line6);
        check_lines_col(&line6.texts, line6.final_text_len,  "    }nr", "nr");
        check_line_final_col(&line6, "nr");

        {
            let let_line = let_data();
            print_line(&let_line);
            let expect_str= "    let a: A  = A;nr";
            check_lines_col(&let_line.texts, let_line.final_text_len,  "    let a = A;nr", expect_str);
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
            check_lines_col(&lines.text, lines.final_text_len,  "    if true {nr", "    if true {...}");
            lines.merge(line4);
            print_lines(&lines);
            check_lines_col(&lines.text, lines.final_text_len,  "    if true {nr    } else {nr", "    if true {...} else {nr");
        }
        {
            /*
             2 |    if a.0 {...} else {...}
             */
            let mut lines = PhantomTextMultiLine::new(line2);
            check_lines_col(&lines.text, lines.final_text_len, "    if true {nr", "    if true {...}");
            // print_lines(&lines);
            // print_line(&line_folded_4);
            lines.merge(line_folded_4);
            // print_lines(&lines);
            check_lines_col(&lines.text, lines.final_text_len,  "    if true {nr    } else {nr", "    if true {...} else {...}");
            lines.merge(line6);
            check_lines_col(&lines.text, lines.final_text_len, "    if true {nr    } else {nr    }nr", "    if true {...} else {...}nr");
        }
    }

    #[test]
    fn check_origin_position_of_final_col() {
        _check_empty_origin_position_of_final_col();
        _check_folded_origin_position_of_final_col();
        _check_let_origin_position_of_final_col();
        _check_folded_origin_position_of_final_col_1();
    }
    fn _check_let_origin_position_of_final_col() {
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
            assert_eq!(let_line.cursor_position_of_final_col(8).1, 8);
        }
        {
            assert_eq!(orgin_text[11], 'A');
            assert_eq!(let_line.cursor_position_of_final_col(11).1, 9);
        }
        {
            assert_eq!(orgin_text[17], ';');
            assert_eq!(let_line.cursor_position_of_final_col(17).1, 13);
        }
        {
            assert_eq!(let_line.cursor_position_of_final_col(30).1, 15);
        }

    }

    fn _check_folded_origin_position_of_final_col_1() {
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
            assert_eq!(line.cursor_position_of_final_col(9), (1, 9));
        }
        {
            let index = 12;
            assert_eq!(orgin_text[index], '{');
            assert_eq!(line.cursor_position_of_final_col(index), (1, 12));
        }
        {
            let index = 19;
            assert_eq!(orgin_text[index], 'l');
            assert_eq!(line.cursor_position_of_final_col(index), (3, 7));
        }
        {
            assert_eq!(line.cursor_position_of_final_col(26), (3, 13));
        }
    }


    fn _check_empty_origin_position_of_final_col() {
        let line = PhantomTextMultiLine::new(empty_data());
        print_lines(&line);
        let orgin_text: Vec<char> = "".chars().into_iter().collect();
        {
            assert_eq!(line.cursor_position_of_final_col(9), (6, 0));
        }
    }
    fn _check_folded_origin_position_of_final_col() {
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
            assert_eq!(line.cursor_position_of_final_col(9), (1, 9));
        }
        {
            assert_eq!(orgin_text[0], ' ');
            assert_eq!(line.cursor_position_of_final_col(0), (1, 0));
        }
        {

            let index = 12;
            assert_eq!(orgin_text[index], '{');
            assert_eq!(line.cursor_position_of_final_col(index), (1, 12));
        }
        {
            let index = 19;
            assert_eq!(orgin_text[index], 'l');
            assert_eq!(line.cursor_position_of_final_col(index), (3, 7));
        }
        {
            let index = 25;
            assert_eq!(orgin_text[index], '.');
            assert_eq!(line.cursor_position_of_final_col(index), (3, 11));
        }
        {
            let index = 29;
            assert_eq!(orgin_text[index], 'r');
            assert_eq!(line.cursor_position_of_final_col(index), (5, 6));
        }

        {
            let index = 40;
            assert_eq!(line.cursor_position_of_final_col(index), (5, 6));
        }

    }


    #[test]
    fn check_final_col_of_col() {
        _check_let_final_col_of_col();
        _check_folded_final_col_of_col();
    }
    fn _check_let_final_col_of_col() {
        let line = PhantomTextMultiLine::new(let_data());
        print_lines(&line);
        let orgin_text: Vec<char> = "    let a: A  = A;nr".chars().into_iter().collect();
        {
            // "0         10        20        30
            // "0123456789012345678901234567890123456789
            // "    let a = A;nr
            // "    let a: A  = A;nr
            // "0123456789012345678901234567890123456789
            // "0         10        20        30
            let orgin_text: Vec<char> = "    let a = A;nr".chars().into_iter().collect();
            let col_line = 6;
            {
                let index = 8;
                assert_eq!(orgin_text[index], 'a');
                assert_eq!(line.final_col_of_col(col_line, index, false), 8);
            }
            {
                let index = 15;
                assert_eq!(orgin_text[index], 'r');
                assert_eq!(line.final_col_of_col(col_line, index, false), 19);
            }
            {
                let index = 18;
                assert_eq!(line.final_col_of_col(col_line, index, false), 19);
            }
        }
    }
    fn _check_folded_final_col_of_col() {
        //  "    if true {...} else {...}nr"
        //  "0123456789012345678901234567890123456789
        //  "0         10        20        30
        let line = get_merged_data();
        print_lines(&line);
        { //  "0         10        20        30
            //  "0123456789012345678901234567890123456789
            //2 "    if true {nr"
            let orgin_text: Vec<char> = "    if true {nr".chars().into_iter().collect();
            let col_line = 1;
            {
                let index = 9;
                assert_eq!(orgin_text[index], 'u');
                assert_eq!(line.final_col_of_col(col_line, index, false), 9);
            }
            {
                let index = 12;
                assert_eq!(orgin_text[index], '{');
                assert_eq!(line.final_col_of_col(col_line, index, false), 11);
            }
            // {
            //     let index = 18;
            //     assert_eq!(line.final_col_of_col(col_line, index, false), 11);
            // }
        }
        {
            //  "0         10        20        30
            //  "0123456789012345678901234567890123456789
            //2 "    } else {nr"
            let col_line = 2;
            {
                let index = 1;
                assert_eq!(line.final_col_of_col(col_line, index, false), 11);
            }
        }
        {
            //  "0         10        20        30
            //  "0123456789012345678901234567890123456789
            //2 "    } else {nr"
            let orgin_text: Vec<char> = "    } else {nr".chars().into_iter().collect();
            let col_line = 3;
            {
                let index = 1;
                assert_eq!(orgin_text[index], ' ');
                assert_eq!(line.final_col_of_col(col_line, index, false), 17);
            }
            {
                let index = 8;
                assert_eq!(orgin_text[index], 's');
                assert_eq!(line.final_col_of_col(col_line, index, false), 20);
            }
            {
                let index = 13;
                assert_eq!(orgin_text[index], 'r');
                assert_eq!(line.final_col_of_col(col_line, index, false), 22);
            }
            {
                let index = 18;
                assert_eq!(line.final_col_of_col(col_line, index, false), 22);
            }
        }
        {
            //  "0         10
            //  "0123456789012
            //2 "    }nr"
            let orgin_text: Vec<char> = "    }nr".chars().into_iter().collect();
            let col_line = 5;
            {
                let index = 1;
                assert_eq!(orgin_text[index], ' ');
                assert_eq!(line.final_col_of_col(col_line, index, false), 28);
            }
            {
                let index = 6;
                assert_eq!(orgin_text[index], 'r');
                assert_eq!(line.final_col_of_col(col_line, index, false), 29);
            }
            {
                let index = 13;
                assert_eq!(line.final_col_of_col(col_line, index, false), 29);
            }
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

    fn check_lines_col(lines: &SmallVec<[Text; 6]>, final_text_len: usize, origin: &str, expect: &str) {
        let rs = combine_with_text(lines,  origin);
        assert_eq!(expect, rs.as_str());
        assert_eq!(final_text_len, expect.len());
    }

    fn check_line_final_col(lines: &PhantomTextLine, rs: &str) {
        for text in &lines.texts {
            if let Text::Phantom {text} = text {
                assert_eq!(text.text.as_str(), sub_str(rs, text.final_col, text.final_col + text.text.len()));
            }
        }
    }

    fn sub_str(text: &str, begin: usize, end: usize) -> &str {
        unsafe {
            text.get_unchecked(begin..end)
        }
    }

    fn print_lines(lines: &PhantomTextMultiLine) {
        println!("PhantomTextMultiLine line={} origin_text_len={} final_text_len={} len_of_line={:?}", lines.line, lines.origin_text_len, lines.final_text_len, lines.len_of_line);
        for text in &lines.text {
            match text {
                Text::Phantom { text } => {
                    println!("Phantom {:?} line={} col={} merge_col={} final_col={} text={} text.len()={}", text.kind, text.line, text.col, text.merge_col, text.final_col, text.text, text.text.len());
                }
                Text::OriginText { text } => {
                    println!("OriginText line={} col={:?} merge_col={:?} final_col={:?}",text.line, text.col, text.merge_col, text.final_col);
                }
                Text::Empty => {
                    println!("Empty");
                }
            }
        }
        println!();
    }

    fn print_line(lines: &PhantomTextLine) {
        println!("PhantomTextLine line={} origin_text_len={} final_text_len={}", lines.line, lines.origin_text_len, lines.final_text_len);
        for text in &lines.texts {
            match text {
                Text::Phantom { text } => {
                    println!("Phantom {:?} line={} col={} merge_col={} final_col={} text={} text.len()={}", text.kind, text.line, text.col, text.merge_col, text.final_col, text.text, text.text.len());
                }
                Text::OriginText { text } => {
                    println!("OriginText line={} col={:?} merge_col={:?} final_col={:?}",text.line, text.col, text.merge_col, text.final_col);
                }
                Text::Empty => {
                    println!("Empty");
                }
            }        }
        println!();
    }



}
