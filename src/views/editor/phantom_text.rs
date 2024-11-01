use std::borrow::Cow;
use std::ops::Range;

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
#[derive(Debug, Clone, Default)]
pub struct PhantomText {
    /// The kind is currently used for sorting the phantom text on a line
    pub kind: PhantomTextKind,
    /// Column on the line that the phantom text should be displayed at
    pub line: usize,
    /// Column on the line that the phantom text should be displayed at.Provided by lsp
    pub col: usize,
    /// Provided by calculate.Column index in final line.
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
    FoldedRangStart {
        same_line: bool,
        end_line: usize,
        end_character: usize,
    },
    CrossLineFoldedRangEnd {
        line: usize,
    },
}

/// Information about the phantom text on a specific line.  
///
/// This has various utility functions for transforming a coordinate (typically a column) into the
/// resulting coordinate after the phantom text is combined with the line's real content.
#[derive(Debug, Default, Clone)]
pub struct PhantomTextLine {
    pub visual_line: usize,
    pub origin_text_len: usize,
    final_text_len: usize,
    /// This uses a smallvec because most lines rarely have more than a couple phantom texts
    pub text: SmallVec<[PhantomText; 6]>,
}

impl PhantomTextLine {
    pub fn new(line: usize,
               origin_text_len: usize, mut text: SmallVec<[PhantomText; 6]>) -> Self {
        text.sort_by(|a, b| {
            if a.col == b.col {
                a.kind.cmp(&b.kind)
            } else {
                a.col.cmp(&b.col)
            }
        });
        let mut phantom_line = Self {
            visual_line: line + 1,
            origin_text_len, final_text_len: origin_text_len, text
        };
        phantom_line.update();
        phantom_line
    }

    pub fn final_text_len(&self) -> usize {
        self.final_text_len
    }
    pub fn log(&self, msg: &str) {
        tracing::info!("{msg} visual_line={} origin_text_len={} final_text_len={}", self.visual_line, self.origin_text_len, self.final_text_len);
        for phantom in &self.text {
            tracing::info!("{:?}", phantom);
        }
        tracing::info!("");
    }

    pub fn update_origin_text_len(&mut self, len: usize) {
        self.origin_text_len = len;
        self.update();
    }
    fn update(&mut self) {
        let mut offset = 0;
        for phantom in &mut self.text {
            match phantom.kind {
                PhantomTextKind::CrossLineFoldedRangEnd{..} => {
                    offset -= phantom.col as i32;
                    phantom.final_col = 0;
                }
                _ => {
                    let final_col = phantom.col as i32 + offset;
                    assert!(final_col >= 0);
                    phantom.final_col = final_col as usize;
                    offset += phantom.text.len() as i32;
                }
            }
        }
        let origin_text_len = self.origin_text_len as i32 + offset;
        assert!(origin_text_len >= 0);
        self.final_text_len = origin_text_len as usize ;
    }
    pub fn add_phantom_style(
        &self,
        attrs_list: &mut AttrsList,
        attrs: Attrs,
        font_size: usize,
        phantom_color: Color,
        collapsed_line_col: usize,
    ) {
        // Apply phantom text specific styling
        for (_offset, size, _, (_line, final_col), phantom) in self.offset_size_iter_2() {
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

            let start = final_col + collapsed_line_col;
            let end = final_col + size + collapsed_line_col;
            // tracing::info!("{} {start}-{end} final_col={final_col} collapsed_line_col={collapsed_line_col} {}", self.visual_line, phantom.text);
            attrs_list.add_span(
                start..end,
                attrs,
            );
        }
    }

    /// 被折叠的范围。用于计算因折叠导致的原始文本的样式变化
    ///
    pub fn floded_ranges(&self) -> Vec<Range<usize>> {
        let mut ranges = Vec::new();
        for item in &self.text {
            if let PhantomTextKind::FoldedRangStart { same_line, end_character, .. } = item.kind {
                if same_line {
                    ranges.push(item.final_col..self.final_text_len);
                } else {
                    ranges.push(item.final_col..end_character);
                }
            } else if let PhantomTextKind::CrossLineFoldedRangEnd{..} = item.kind {
                ranges.push(0..item.final_col);
            }
        }
        ranges
    }

    /// Translate a column position into the text into what it would be after combining
    /// 求原始文本在最终文本的位置。场景：计算原始文本的样式在最终文本的位置。
    ///
    /// 最终文本的位置 = 原始文本位置 + 之前的幽灵文本长度
    ///
    /// todo remove Option
    pub fn col_at(&self, pre_col: usize) -> Option<usize> {
        // if self.visual_line == 11 {
        //     tracing::info!("11");
        // }
        let mut last_offset = 0;
        for (_col_shift, _size, (_line, col), (__final_line, final_col), _phantom) in self.offset_size_iter_2() {
            // if self.visual_line == 11 {
            //     tracing::info!("{pre_col} {col_shift} {size} {_line} {col} {line} {final_col} {:?}", _phantom);
            // }
            if pre_col <= col {
                break;
            }
            last_offset = final_col as i32 - col as i32;
        }
        // if self.visual_line == 11 {
        //     tracing::info!("\n");
        // }
        let final_pre_col = pre_col as i32 + last_offset;
        if final_pre_col < 0 {
            return None
        } else {
            return Some(final_pre_col as usize);
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
        let mut line = self.visual_line - 1;
        // (最终文本上该幽灵文本前其他幽灵文本的总长度，幽灵文本的长度，幽灵文本在原始文本的字符位置，幽灵文本)
        for (mut col_shift, size, (_, hint_col), phantom) in self.offset_size_iter() {
            if let PhantomTextKind::FoldedRangStart { same_line, end_line, end_character } = &phantom.kind {
                if !same_line {
                    line = *end_line as usize;
                }
            }
            if self.visual_line == 10 {
                tracing::info!("col_shift={col_shift} size={size} hint_col={hint_col} {phantom:?}");
            }
            // if col_shift < 0 {
            //     col_shift = 0;
            // }
            let shifted_start = hint_col + col_shift as usize;
            let shifted_end = hint_col + col_shift as usize + size as usize;

            if col >= shifted_start {
                if col >= shifted_end {
                    last = col - col_shift as usize - size as usize;
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
    pub fn before_col_2(&self, col: usize, tracing: bool) -> (usize, usize) {

        if tracing {
            tracing::info!("col={col} {}", self.final_text_len);
            self.log("before_col_2");
        }

        let col = col.min(self.final_text_len);
        let mut last_line = self.visual_line - 1;
        let mut last_final_col = 0;
        let mut last_origin_col = 0;
        // (最终文本上该幽灵文本前其他幽灵文本的总长度，幽灵文本的长度，幽灵文本在原始文本的字符位置，幽灵文本)
        for (col_shift, size, (origin_line, origin_col), (final_line, final_col), phantom) in self.offset_size_iter_2() {
            let shifted_start = final_col;
            let shifted_end = final_col + size;
            if col >= shifted_end {
                // continue to judge next phantom
                last_line = origin_line;
                last_final_col = final_col + size;
                last_origin_col = origin_col;
            // } else if col == shifted_end {
            //     // shifted_end is the cursor should be locate.
            //     last_col_shift = col_shift + size;
            } else if shifted_start < col && col < shifted_end {
                return (origin_line, origin_col);
            } else if col < shifted_start {
                return (last_line, col - last_final_col + last_origin_col);
            }

        }
        return (last_line, col - last_final_col + last_origin_col);
    }

    /// Insert the hints at their positions in the text
    /// Option<(collapsed line, collapsed col index)>
    pub fn combine_with_text<'a>(&self, text: &'a str) -> (Cow<'a, str>, Option<(usize, usize)>) {
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
                        &text_o.subseq(Interval::new(end_character, text_o.len())),
                    );
                    col_shift = col_shift + phantom.text.len() as i32
                        - (end_character - location) as i32;
                } else {
                    text_o = text_o.subseq(Interval::new(0, location));
                    text_o.push_str(&phantom.text);
                    text = Cow::Owned(text_o);
                    return (text, Some((end_line, end_character)));
                }
            } else if let PhantomTextKind::CrossLineFoldedRangEnd{..} = phantom.kind {
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
    /// (最终文本上该幽灵文本前其他幽灵文本的总长度，幽灵文本的长度，(幽灵文本在原始文本的字符位置), (幽灵文本在最终文本的字符位置)，幽灵文本)
    ///
    /// 所以原始文本在最终文本的位置= 原始位置 + 之前的幽灵文本总长度
    ///
    pub fn offset_size_iter(&self) -> impl Iterator<Item = (usize, usize, (usize, usize), &PhantomText)> + '_ {
        let mut col_shift = 0usize;
        let mut line = self.visual_line - 1;
        self.text.iter().map(move |phantom| {
            let rs = match phantom.kind {
                PhantomTextKind::FoldedRangStart {
                    same_line,
                    end_line, end_character,
                } => {
                    let pre_col_shift = col_shift;
                    let phantom_line = line;
                    if same_line {
                        col_shift = col_shift + phantom.text.len()
                            - (end_character - phantom.col) ;
                    } else {
                        line = end_line;
                        col_shift += phantom.text.len();
                    }
                    (
                        pre_col_shift,
                        phantom.text.len(),
                        (phantom_line, phantom.col),
                        phantom,
                    )
                }
                PhantomTextKind::CrossLineFoldedRangEnd {..} => {
                    // col_shift -= phantom.col;
                    (0, 0, (line, phantom.col), phantom)
                }
                _ => {
                    let pre_col_shift = col_shift;
                    col_shift += phantom.text.len();
                    (
                        pre_col_shift,
                        phantom.text.len(),
                        (line, phantom.col),
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

    /// (最终文本上该幽灵文本前其他幽灵文本的总长度，幽灵文本的长度，(幽灵文本在原始文本的字符位置), (幽灵文本在最终文本的字符位置)，幽灵文本)
    ///
    /// 所以原始文本在最终文本的位置= 原始位置 + 之前的幽灵文本总长度
    ///
    pub fn offset_size_iter_2(&self) -> impl Iterator<Item = (usize, usize, (usize, usize), (usize, usize), &PhantomText)> + '_ {
        let mut before_phantom_len = 0usize;
        let line = self.visual_line - 1;
        self.text.iter().map(move |phantom| {
            match phantom.kind {
                PhantomTextKind::CrossLineFoldedRangEnd{..} => {
                    (before_phantom_len, 0, (phantom.line, phantom.col), (line, phantom.final_col), phantom)
                }
                _ => {
                    let current_before_phantom_len = before_phantom_len;
                    before_phantom_len += phantom.text.len();
                    (
                        current_before_phantom_len,
                        phantom.text.len(),
                        (phantom.line, phantom.col), (line, phantom.final_col),
                        phantom,
                    )
                }
            }
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

#[cfg(test)]
mod test {
    use smallvec::SmallVec;
    use crate::views::editor::phantom_text::{PhantomText, PhantomTextKind, PhantomTextLine};
    use std::default::Default;
    /**
    9 |    if a.0 {
    10|      println!("start");
    11|    } else {
    12|     println!("end");
    13|    }
     **/
/**
9 |    if a.0 {...} else {
 **/
    fn init_folded_line() -> PhantomTextLine{
        let mut text: SmallVec<[PhantomText; 6]> = SmallVec::new();
        text.push(PhantomText{
            kind: PhantomTextKind::FoldedRangStart {
                same_line: false,
                end_line: 10,
                end_character: 5,
            }, line:8, final_col: 11,
            col: 11,
            text: "{...}".to_string(), ..Default::default()
        });
        // text.push(PhantomText{
        //     kind: PhantomTextKind::CrossLineFoldedRangEnd,
        //     col: 5,
        //     text: "".to_string(), ..Default::default()
        // });
        PhantomTextLine {
            visual_line: 9,
            text,
        }
    }
    /**
     **9 |    if a.0 {...} else {...}
     **/
    fn init_folded_folded_line() -> PhantomTextLine{
        let mut text_line = init_folded_line();
        text_line.text.push(PhantomText{
            kind: PhantomTextKind::FoldedRangStart {
                same_line: false,
                end_line: 12,
                end_character: 5,
            },
            line: 10, final_col: 22,
            col: 11,
            text: "{...}".to_string(), ..Default::default()
        });
        // text_line.text.push(PhantomText{
        //     kind: PhantomTextKind::CrossLineFoldedRangEnd,
        //     col: 5,
        //     text: "".to_string(), ..Default::default()
        // });
        text_line
    }
    #[test]
    fn test_offset_size_iter() {
        /*
         9 |    if a.0 {...} else {...}
         */
        let text_line = init_folded_folded_line();
        let mut offset = text_line.offset_size_iter_2();
        let item = offset.next();
        assert_eq!(item.map(|x| (x.0, x.1, x.2, x.3)), Some((0, 5, (8, 11), (8, 11))));
        let item = offset.next();
        assert_eq!(item.map(|x| (x.0, x.1, x.2, x.3)), Some((5, 5, (10, 11), (8, 22))));
        // while let Some((total_prev_ghostLength, ghost_text_len, ghost_text_position, ghost_text)) = offset.next() {
        //
        // }
    }
}
