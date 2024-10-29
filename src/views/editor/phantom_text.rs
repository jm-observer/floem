use std::borrow::Cow;

use crate::{
    peniko::Color,
    text::{Attrs, AttrsList},
};
use floem_editor_core::cursor::CursorAffinity;
use lapce_xi_rope::{tree::Leaf, Interval};
use smallvec::SmallVec;

/// `PhantomText` is for text that is not in the actual document, but should be rendered with it.
///
/// Ex: Inlay hints, IME text, error lens' diagnostics, etc
#[derive(Debug, Clone)]
pub struct PhantomText {
    /// The kind is currently used for sorting the phantom text on a line
    pub kind: PhantomTextKind,
    /// Column on the line that the phantom text should be displayed at
    pub col: usize,
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

#[derive(Debug, Clone, Copy, Ord, Eq, PartialEq, PartialOrd)]
pub enum PhantomTextKind {
    /// Input methods
    Ime,
    Placeholder,
    /// Completion lens / Inline completion
    Completion,
    /// Inlay hints supplied by an LSP/PSP (like type annotations)
    InlayHint,
    /// Error lens
    Diagnostic,
    FoldedRangStart {
        same_line: bool,
        end_line: u32,
        end_character: u32,
    },
    CrossLineFoldedRangEnd,
}

/// Information about the phantom text on a specific line.  
///
/// This has various utility functions for transforming a coordinate (typically a column) into the
/// resulting coordinate after the phantom text is combined with the line's real content.
#[derive(Debug, Default, Clone)]
pub struct PhantomTextLine {
    pub visual_line: usize,
    /// This uses a smallvec because most lines rarely have more than a couple phantom texts
    pub text: SmallVec<[PhantomText; 6]>,
}

impl PhantomTextLine {
    pub fn add_phantom_style(
        &self,
        attrs_list: &mut AttrsList,
        attrs: Attrs,
        font_size: usize,
        phantom_color: Color,
        collapsed_line_col: usize,
    ) {
        // Apply phantom text specific styling
        for (offset, size, col, phantom) in self.offset_size_iter() {
            if offset < 0 {
                tracing::debug!("offset < 0 {:?}", phantom);
                continue;
            }
            if size < 0 {
                tracing::debug!("size < 0 {:?}", phantom.kind);
                assert_eq!(phantom.kind, PhantomTextKind::CrossLineFoldedRangEnd);
                continue;
            }

            let offset = offset as usize;
            let size = size as usize;

            let start = col + offset;
            let end = start + size;

            // tracing::info!(
            //     "offset={offset} size={size} col={col} text={}",
            //     phantom.text
            // );

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
                (start + collapsed_line_col)..(end + collapsed_line_col),
                attrs,
            );
        }
    }

    /// Translate a column position into the text into what it would be after combining
    /// 求原始文本在最终文本的位置
    pub fn col_at(&self, pre_col: usize) -> Option<usize> {
        let pre_col = pre_col as i32;
        let mut last = pre_col;
        for (col_shift, size, col, phantom) in self.offset_size_iter() {
            // (最终文本上该幽灵文本前其他幽灵文本的总长度，幽灵文本的长度，幽灵文本在原始文本的字符位置，幽灵文本)
            // 所以原始文本在最终文本的位置= 原始位置 + 之前的幽灵文本总长度
            if col_shift < 0 {
                tracing::debug!("offset < 0 {:?}", phantom);
            }
            if size < 0 {
                tracing::debug!("size < 0 {:?}", phantom.kind);
                assert_eq!(phantom.kind, PhantomTextKind::CrossLineFoldedRangEnd);
            }
            let col = col as i32;
            // tracing::info!("pre_col={pre_col} = col_shift={col_shift} size={size} col={col} {}", phantom.text);
            if pre_col >= col {
                last = pre_col + col_shift + size;
            } else {
                break;
            }
        }
        if last < 0 {
            None
        } else {
            Some(last as usize)
        }
    }

    /// Translate a column position into the text into what it would be after combining
    ///
    /// 将列位置转换为合并后的文本位置
    ///
    /// If `before_cursor` is false and the cursor is right at the start then it will stay there
    /// (Think 'is the phantom text before the cursor')
    pub fn col_after(&self, pre_col: usize, before_cursor: bool) -> usize {
        let mut last = pre_col;
        for (col_shift, size, col, text) in self.offset_size_iter() {
            if col_shift < 0 {
                tracing::warn!("offset < 0 {:?}", text);
                // continue;
            }
            if size < 0 {
                // tracing::debug!("size < 0 {:?}", text.kind);
                assert_eq!(text.kind, PhantomTextKind::CrossLineFoldedRangEnd);
                // continue;
            }

            let before_cursor = match text.affinity {
                Some(CursorAffinity::Forward) => true,
                Some(CursorAffinity::Backward) => false,
                None => before_cursor,
            };

            if pre_col > col || (pre_col == col && before_cursor) {
                last = pre_col + col_shift as usize + size as usize;
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
        for (col_shift, size, col, text) in self.offset_size_iter() {
            if col_shift < 0 {
                tracing::warn!("offset < 0 {:?}", text);
                // continue;
            }
            if size < 0 {
                tracing::debug!("size < 0 {:?}", text.kind);
                assert_eq!(text.kind, PhantomTextKind::CrossLineFoldedRangEnd);
                // continue;
            }
            if pre_col > col || (pre_col == col && before_cursor) {
                last = pre_col + col_shift as usize + size as usize;
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
        for (col_shift, size, col, phantom) in self.offset_size_iter() {
            if skip(phantom) {
                continue;
            }
            if col_shift < 0 {
                tracing::warn!("offset < 0 {:?}", phantom);
                continue;
            }
            if size < 0 {
                tracing::debug!("size < 0 {:?}", phantom.kind);
                assert_eq!(phantom.kind, PhantomTextKind::CrossLineFoldedRangEnd);
                continue;
            }

            if pre_col > col || (pre_col == col && before_cursor) {
                last = pre_col + col_shift as usize + size as usize;
            }
        }

        last
    }

    /// Translate a column position into the position it would be before combining
    ///
    /// 将列位置转换为合并前的位置，也就是原始文本的位置？意义？
    ///
    /// ????????
    pub fn before_col(&self, col: usize) -> usize {
        let mut last = col;
        // (最终文本上该幽灵文本前其他幽灵文本的总长度，幽灵文本的长度，幽灵文本在原始文本的字符位置，幽灵文本)
        for (mut col_shift, size, hint_col, phantom) in self.offset_size_iter() {
            if self.visual_line == 10 {
                tracing::info!("col_shift={col_shift} size={size} hint_col={hint_col} {phantom:?}");
                // continue;
            }
            if col_shift < 0 {
                col_shift = 0;
            }
            if size < 0 {
                tracing::debug!("size < 0 {:?}", phantom.kind);
                assert_eq!(phantom.kind, PhantomTextKind::CrossLineFoldedRangEnd);
                continue;
            }
            let shifted_start = hint_col + col_shift as usize;
            let shifted_end = hint_col + col_shift as usize + size as usize;

            if col >= shifted_start {
                if col >= shifted_end {
                    last = col - col_shift as usize - size as usize;
                } else {
                    return hint_col;
                    // last = hint_col;
                }
            } else {
                return last;
            }
        }
        last
    }

    /// Insert the hints at their positions in the text
    /// Option<(collapsed line, collapsed col index)>
    pub fn combine_with_text<'a>(&self, text: &'a str) -> (Cow<'a, str>, Option<(u32, u32)>) {
        let mut text = Cow::Borrowed(text);
        let mut col_shift: i32 = 0;

        for phantom in self.text.iter() {
            let location = phantom.col as i32 + col_shift;
            if location < 0 {
                tracing::error!("{:?} {}", phantom.kind, phantom.text);
                continue;
            }
            let location = location as usize;
            // Stop iterating if the location is bad
            if text.get(location..).is_none() {
                return (text, None);
            }

            let mut text_o = text.into_owned();

            if let PhantomTextKind::FoldedRangStart {
                same_line,
                end_character,
                end_line,
            } = phantom.kind
            {
                if same_line {
                    let mut new_text_o = text_o.subseq(Interval::new(0, location));
                    new_text_o.push_str(&phantom.text);
                    new_text_o.push_str(
                        &text_o.subseq(Interval::new(end_character as usize, text_o.len())),
                    );
                    col_shift = col_shift + phantom.text.len() as i32
                        - (end_character as usize - location) as i32;
                } else {
                    text_o = text_o.subseq(Interval::new(0, location));
                    text_o.push_str(&phantom.text);
                    text = Cow::Owned(text_o);
                    return (text, Some((end_line, end_character)));
                }
            } else if let PhantomTextKind::CrossLineFoldedRangEnd = phantom.kind {
                text_o = text_o.subseq(Interval::new(location, text_o.len()));
                col_shift -= location as i32;
            } else {
                text_o.insert_str(location, &phantom.text);
                col_shift += phantom.text.len() as i32;
            }

            text = Cow::Owned(text_o);
        }

        (text, None)
    }

    /// Iterator over (col_shift, size, hint, pre_column)
    /// Note that this only iterates over the ordered text, since those depend on the text for where
    /// they'll be positioned
    /// (finally col index, phantom len, phantom at origin text index, phantom)
    ///
    /// (最终文本上该幽灵文本前其他幽灵文本的总长度，幽灵文本的长度，幽灵文本在原始文本的字符位置，幽灵文本)
    ///
    /// 所以原始文本在最终文本的位置= 原始位置 + 之前的幽灵文本总长度
    ///
    pub fn offset_size_iter(&self) -> impl Iterator<Item = (i32, i32, usize, &PhantomText)> + '_ {
        let mut col_shift = 0i32;
        self.text.iter().map(move |phantom| {
            let rs = match phantom.kind {
                PhantomTextKind::FoldedRangStart {
                    same_line,
                    end_character,
                    ..
                } => {
                    let pre_col_shift = col_shift;
                    if same_line {
                        col_shift = col_shift + phantom.text.len() as i32
                            - (end_character as usize - phantom.col) as i32;
                    } else {
                        col_shift += phantom.text.len() as i32;
                    }

                    (
                        pre_col_shift,
                        phantom.text.len() as i32,
                        phantom.col,
                        phantom,
                    )
                }
                PhantomTextKind::CrossLineFoldedRangEnd => {
                    col_shift -= phantom.col as i32;
                    (0, -(phantom.col as i32), phantom.col, phantom)
                }
                _ => {
                    let pre_col_shift = col_shift;
                    col_shift += phantom.text.len() as i32;
                    (
                        pre_col_shift,
                        phantom.text.len() as i32,
                        phantom.col,
                        phantom,
                    )
                }
            };
            tracing::debug!(
                "visual_line={} offset={} len={} col={} text={} {:?}",
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
        for (offset, size, col, phantom) in self.offset_size_iter() {
            if offset < 0 {
                tracing::warn!("apply_attr_styles offset < 0 {:?}", phantom);
                continue;
            }
            if size < 0 {
                tracing::debug!("apply_attr_styles size < 0 {:?}", phantom.kind);
                assert_eq!(phantom.kind, PhantomTextKind::CrossLineFoldedRangEnd);
                continue;
            }
            let start = col + offset as usize;
            let end = start + size as usize;

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
}
