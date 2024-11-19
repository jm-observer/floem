use std::{
    cell::{Cell},
    cmp::Ordering,
    collections::{hash_map::DefaultHasher},
    hash::{Hash, Hasher},
    rc::Rc,
    sync::Arc,
    time::Duration,
};

use lapce_xi_rope::Rope;

pub use floem_editor_core as core;
use floem_editor_core::{
    buffer::rope_text::{RopeText, RopeTextVal},
    command::MoveCommand,
    cursor::{ColPosition, Cursor, CursorAffinity, CursorMode},
    mode::Mode,
    movement::Movement,
    register::Register,
    selection::Selection,
};
use floem_reactive::{SignalGet, SignalTrack, SignalUpdate, SignalWith, Trigger};
use floem_renderer::text::FONT_SYSTEM;
pub use prop::*;

use crate::{
    action::{exec_after, TimerToken},
    keyboard::Modifiers,
    kurbo::{Point, Rect, Vec2},
    peniko::Color,
    pointer::{PointerButton, PointerInputEvent, PointerMoveEvent},
    reactive::{batch, ReadSignal, RwSignal, Scope},
    text::{Attrs, AttrsList, LineHeightValue, TextLayout, Wrap},
};
use crate::views::editor::lines::{OriginFoldedLine};
use crate::views::editor::phantom_text::PhantomTextMultiLine;

use self::{
    command::Command,
    id::EditorId,
    layout::TextLayoutLine,
    text::{Document, Preedit, PreeditData, Styling, WrapMethod},
    view::{ScreenLines, ScreenLinesBase},
    visual_line::{
        ConfigId, FontSizeCacheId, hit_position_aff, ResolvedWrap, RVLine,
        TextLayoutProvider, VLine, VLineInfo,
    },
};

pub mod actions;
pub mod color;
pub mod command;
pub mod gutter;
pub mod id;
pub mod keypress;
pub mod layout;
pub mod lines;
pub mod listener;
pub mod movement;
pub mod phantom_text;
mod prop;
pub mod text;
pub mod text_document;
pub mod view;
pub mod visual_line;

pub(crate) const CHAR_WIDTH: f64 = 7.5;

/// The main structure for the editor view itself.  
/// This can be considered to be the data part of the `View`.
/// It holds an `Rc<dyn Document>` within as the document it is a view into.  
#[derive(Clone)]
pub struct Editor {
    pub cx: Cell<Scope>,
    effects_cx: Cell<Scope>,

    id: EditorId,

    pub active: RwSignal<bool>,

    /// Whether you can edit within this editor.
    pub read_only: RwSignal<bool>,

    pub(crate) doc: RwSignal<Rc<dyn Document>>,

    pub cursor: RwSignal<Cursor>,

    pub window_origin: RwSignal<Point>,
    pub viewport: RwSignal<Rect>,
    pub parent_size: RwSignal<Rect>,

    pub editor_view_focused: Trigger,
    pub editor_view_focus_lost: Trigger,
    pub editor_view_id: RwSignal<Option<crate::id::ViewId>>,

    /// The current scroll position.
    pub scroll_delta: RwSignal<Vec2>,
    pub scroll_to: RwSignal<Option<Vec2>>,

    /// Holds the cache of the lines and provides many utility functions for them.
    lines: RwSignal<lines::Lines>,
    pub screen_lines: RwSignal<ScreenLines>,

    /// Modal mode register
    pub register: RwSignal<Register>,
    /// Cursor rendering information, such as the cursor blinking state.
    pub cursor_info: CursorInfo,

    pub last_movement: RwSignal<Movement>,

    /// Whether ime input is allowed.  
    /// Should not be set manually outside of the specific handling for ime.
    pub ime_allowed: RwSignal<bool>,

    /// The Editor Style
    pub es: RwSignal<EditorStyle>,

    pub floem_style_id: RwSignal<u64>,
}
impl Editor {
    /// Create a new editor into the given document, using the styling.  
    /// `doc`: The backing [`Document`], such as [TextDocument](self::text_document::TextDocument)
    /// `style`: How the editor should be styled, such as [SimpleStyling](self::text::SimpleStyling)
    // pub fn new(cx: Scope, doc: Rc<dyn Document>, style: Rc<dyn Styling>, modal: bool) -> Editor {
    //     let id = doc.editor_id();
    //     Editor::new_id(cx, id, doc, style, modal)
    // }

    /// Create a new editor into the given document, using the styling.  
    /// `id` should typically be constructed by [`EditorId::next`]  
    /// `doc`: The backing [`Document`], such as [TextDocument](self::text_document::TextDocument)
    /// `style`: How the editor should be styled, such as [SimpleStyling](self::text::SimpleStyling)
    pub fn new(
        cx: Scope,
        doc: Rc<dyn Document>,
        modal: bool
    ) -> Editor {
        let editor = Editor::new_direct(cx, doc, modal);
        editor.recreate_view_effects();

        editor
    }

    // TODO: shouldn't this accept an `RwSignal<Rc<dyn Document>>` so that it can listen for
    // changes in other editors?
    // TODO: should we really allow callers to arbitrarily specify the Id? That could open up
    // confusing behavior.

    /// Create a new editor into the given document, using the styling.  
    /// `id` should typically be constructed by [`EditorId::next`]  
    /// `doc`: The backing [`Document`], such as [TextDocument](self::text_document::TextDocument)
    /// `style`: How the editor should be styled, such as [SimpleStyling](self::text::SimpleStyling)
    /// This does *not* create the view effects. Use this if you're creating an editor and then
    /// replacing signals. Invoke [`Editor::recreate_view_effects`] when you are done.
    /// ```rust,ignore
    /// let shared_scroll_beyond_last_line = /* ... */;
    /// let editor = Editor::new_direct(cx, id, doc, style);
    /// editor.scroll_beyond_last_line.set(shared_scroll_beyond_last_line);
    /// ```
    pub fn new_direct(
        cx: Scope,
        doc: Rc<dyn Document>,
        modal: bool
    ) -> Editor {
        let id = doc.editor_id();
        let es = doc.editor_style();
        let viewport = doc.viewport();
        let cx = cx.create_child();

        let cursor_mode = if modal {
            CursorMode::Normal(0)
        } else {
            CursorMode::Insert(Selection::caret(0))
        };
        let cursor = Cursor::new(cursor_mode, None, None);
        let cursor = cx.create_rw_signal(cursor);
        let lines = doc.lines();
        let doc = cx.create_rw_signal(doc);
        // let font_sizes = Rc::new(EditorFontSizes {
        //     id,
        //     style: style.read_only(),
        //     doc: doc.read_only(),
        // });
        // let lines = Rc::new(Lines::new(cx, font_sizes));

        let screen_lines = cx.create_rw_signal(ScreenLines::new(cx, viewport.get_untracked()));


        let ed = Editor {
            cx: Cell::new(cx),
            effects_cx: Cell::new(cx.create_child()),
            id,
            active: cx.create_rw_signal(false),
            read_only: cx.create_rw_signal(false),
            doc,
            cursor,
            window_origin: cx.create_rw_signal(Point::ZERO),
            viewport,
            parent_size: cx.create_rw_signal(Rect::ZERO),
            scroll_delta: cx.create_rw_signal(Vec2::ZERO),
            scroll_to: cx.create_rw_signal(None),
            editor_view_focused: cx.create_trigger(),
            editor_view_focus_lost: cx.create_trigger(),
            editor_view_id: cx.create_rw_signal(None),
            lines,
            screen_lines,
            register: cx.create_rw_signal(Register::default()),
            cursor_info: CursorInfo::new(cx),
            last_movement: cx.create_rw_signal(Movement::Left),
            ime_allowed: cx.create_rw_signal(false),
            es,
            floem_style_id: cx.create_rw_signal(0),
        };

        create_view_effects(ed.effects_cx.get(), &ed);

        ed
    }

    pub fn id(&self) -> EditorId {
        self.id
    }

    /// Get the document untracked
    pub fn doc(&self) -> Rc<dyn Document> {
        self.doc.get_untracked()
    }

    pub fn doc_track(&self) -> Rc<dyn Document> {
        self.doc.get()
    }

    // TODO: should this be `ReadSignal`? but read signal doesn't have .track
    pub fn doc_signal(&self) -> RwSignal<Rc<dyn Document>> {
        self.doc
    }

    pub fn config_id(&self) -> ConfigId {
        let style_id = self.doc.with(|s| s.id());
        let floem_style_id = self.floem_style_id;
        ConfigId::new(style_id, floem_style_id.get_untracked())
    }

    pub fn recreate_view_effects(&self) {
        batch(|| {
            self.effects_cx.get().dispose();
            self.effects_cx.set(self.cx.get().create_child());
            create_view_effects(self.effects_cx.get(), self);
        });
    }

    /// Swap the underlying document out
    pub fn update_doc(&self, doc: Rc<dyn Document>) {
        batch(|| {
            // Get rid of all the effects
            self.effects_cx.get().dispose();

            // *self.lines.font_sizes.borrow_mut() = Rc::new(EditorFontSizes {
            //     id: self.id(),
            //     style: self.style.read_only(),
            //     doc: self.doc.read_only(),
            // });
            // self.lines.clear(0, None);

            // let font_sizes = Rc::new(EditorFontSizes {
            //     id: self.id(),
            //     style: self.style.read_only(),
            //     doc: self.doc.read_only(),
            // });
            self.doc.set(doc);
            let ed = self.clone();
            self.lines.update(|x| {
                x.update(&ed);
            });
            self.screen_lines.update(|screen_lines| {
                screen_lines.clear(self.viewport.get_untracked());
            });

            // Recreate the effects
            self.effects_cx.set(self.cx.get().create_child());
            create_view_effects(self.effects_cx.get(), self);
        });
    }

    // pub fn update_styling(&self, styling: Rc<dyn Styling>) {
    //     batch(|| {
    //         // Get rid of all the effects
    //         self.effects_cx.get().dispose();
    //
    //         // let font_sizes = Rc::new(EditorFontSizes {
    //         //     id: self.id(),
    //         //     style: self.style.read_only(),
    //         //     doc: self.doc.read_only(),
    //         // });
    //
    //         let ed = self.clone();
    //         self.lines.update(|x| {
    //             x.update(&ed);
    //         });
    //         //
    //         // *self.lines.font_sizes.borrow_mut() =
    //         // self.lines.clear(0, None);
    //
    //         self.style.set(styling);
    //
    //         self.screen_lines.update(|screen_lines| {
    //             screen_lines.clear(self.viewport.get_untracked());
    //         });
    //
    //         // Recreate the effects
    //         self.effects_cx.set(self.cx.get().create_child());
    //         create_view_effects(self.effects_cx.get(), self);
    //     });
    // }

    // pub fn duplicate(&self, editor_id: Option<EditorId>) -> Editor {
    //     let doc = self.doc();
    //     let style = self.style();
    //     let mut editor = Editor::new_direct(
    //         self.cx.get(),
    //         editor_id.unwrap_or_else(EditorId::next),
    //         doc,
    //         style,
    //         false,
    //     );
    //
    //     batch(|| {
    //         editor.read_only.set(self.read_only.get_untracked());
    //         editor.es.set(self.es.get_untracked());
    //         editor
    //             .floem_style_id
    //             .set(self.floem_style_id.get_untracked());
    //         editor.cursor.set(self.cursor.get_untracked());
    //         editor.scroll_delta.set(self.scroll_delta.get_untracked());
    //         editor.scroll_to.set(self.scroll_to.get_untracked());
    //         editor.window_origin.set(self.window_origin.get_untracked());
    //         editor.viewport.set(self.viewport.get_untracked());
    //         editor.parent_size.set(self.parent_size.get_untracked());
    //         editor.register.set(self.register.get_untracked());
    //         editor.cursor_info = self.cursor_info.clone();
    //         editor.last_movement.set(self.last_movement.get_untracked());
    //         // ?
    //         // editor.ime_allowed.set(self.ime_allowed.get_untracked());
    //     });
    //
    //     editor.recreate_view_effects();
    //
    //     editor
    // }

    // /// Get the styling untracked
    // pub fn style(&self) -> Rc<dyn Styling> {
    //     self.doc.get_untracked()
    // }

    /// Get the text of the document  
    /// You should typically prefer [`Self::rope_text`]
    pub fn text(&self) -> Rope {
        self.doc().text()
    }

    /// Get the [`RopeTextVal`] from `doc` untracked
    pub fn rope_text(&self) -> RopeTextVal {
        self.doc().rope_text()
    }

    pub fn update_lines(&self) {
        let ed = self.clone();
        batch(|| {
            if ed.lines.try_update(|x| x.update(&ed)).unwrap_or(false) {
                ed.screen_lines.update(|screen_lines| {
                    let new_screen_lines =
                        ed.compute_screen_lines(screen_lines.base);
                    *screen_lines = new_screen_lines;
                });
            }
        });

    }



    pub fn vline_infos(&self, start: usize, end: usize) -> Vec<VLineInfo<VLine>> {
        self.lines.with_untracked(|x| x.vline_infos(start, end))
    }

    pub fn text_prov(&self) -> &Self {
        self
    }

    fn preedit(&self) -> PreeditData {
        self.doc.with_untracked(|doc| doc.preedit())
    }

    pub fn set_preedit(&self, text: String, cursor: Option<(usize, usize)>, offset: usize) {
        batch(|| {
            self.preedit().preedit.set(Some(Preedit {
                text,
                cursor,
                offset,
            }));

            self.doc().cache_rev().update(|cache_rev| {
                *cache_rev += 1;
            });
        });
    }

    pub fn clear_preedit(&self) {
        let preedit = self.preedit();
        if preedit.preedit.with_untracked(|preedit| preedit.is_none()) {
            return;
        }

        batch(|| {
            preedit.preedit.set(None);
            self.doc().cache_rev().update(|cache_rev| {
                *cache_rev += 1;
            });
        });
    }

    pub fn receive_char(&self, c: &str) {
        self.doc().receive_char(self, c)
    }

    fn compute_screen_lines(&self, base: RwSignal<ScreenLinesBase>) -> ScreenLines {
        // This function *cannot* access `ScreenLines` with how it is currently implemented.
        // This is being called from within an update to screen lines.

        self.doc().compute_screen_lines(self, base)
    }

    /// Default handler for `PointerDown` event
    pub fn pointer_down(&self, pointer_event: &PointerInputEvent) {
        match pointer_event.button {
            PointerButton::Primary => {
                self.active.set(true);
                self.left_click(pointer_event);
            }
            PointerButton::Secondary => {
                self.right_click(pointer_event);
            }
            _ => {}
        }
    }

    pub fn left_click(&self, pointer_event: &PointerInputEvent) {
        match pointer_event.count {
            1 => {
                self.single_click(pointer_event);
            }
            2 => {
                self.double_click(pointer_event);
            }
            3 => {
                self.triple_click(pointer_event);
            }
            _ => {}
        }
    }

    pub fn single_click(&self, pointer_event: &PointerInputEvent) {
        let mode = self.cursor.with_untracked(|c| c.get_mode());
        let (new_offset, _) = self.offset_of_point(mode, pointer_event.pos, true);
        self.cursor.update(|cursor| {
            cursor.set_offset(
                new_offset,
                pointer_event.modifiers.shift(),
                pointer_event.modifiers.alt(),
            )
        });
    }

    pub fn double_click(&self, pointer_event: &PointerInputEvent) {
        let mode = self.cursor.with_untracked(|c| c.get_mode());
        let (mouse_offset, _) = self.offset_of_point(mode, pointer_event.pos, false);
        let (start, end) = self.select_word(mouse_offset);

        self.cursor.update(|cursor| {
            cursor.add_region(
                start,
                end,
                pointer_event.modifiers.shift(),
                pointer_event.modifiers.alt(),
            )
        });
    }

    pub fn triple_click(&self, pointer_event: &PointerInputEvent) {
        let mode = self.cursor.with_untracked(|c| c.get_mode());
        let (mouse_offset, _) = self.offset_of_point(mode, pointer_event.pos, false);
        let vline = self.visual_line_of_offset(mouse_offset, CursorAffinity::Backward).0;

        self.cursor.update(|cursor| {
            cursor.add_region(
                vline.interval.start,
                vline.interval.end,
                pointer_event.modifiers.shift(),
                pointer_event.modifiers.alt(),
            )
        });
    }

    pub fn pointer_move(&self, pointer_event: &PointerMoveEvent) {
        let mode = self.cursor.with_untracked(|c| c.get_mode());
        let (offset, _is_inside) = self.offset_of_point(mode, pointer_event.pos, false);
        if self.active.get_untracked() && self.cursor.with_untracked(|c| c.offset()) != offset {
            self.cursor
                .update(|cursor| cursor.set_offset(offset, true, pointer_event.modifiers.alt()));
        }
    }

    pub fn pointer_up(&self, _pointer_event: &PointerInputEvent) {
        self.active.set(false);
    }

    fn right_click(&self, pointer_event: &PointerInputEvent) {
        let mode = self.cursor.with_untracked(|c| c.get_mode());
        let (offset, _) = self.offset_of_point(mode, pointer_event.pos, false);
        let doc = self.doc();
        let pointer_inside_selection = self
            .cursor
            .with_untracked(|c| c.edit_selection(&doc.rope_text()).contains(offset));
        if !pointer_inside_selection {
            // move cursor to pointer position if outside current selection
            self.single_click(pointer_event);
        }
    }

    // TODO: should this have modifiers state in its api
    pub fn page_move(&self, down: bool, mods: Modifiers) {
        let viewport = self.viewport.get_untracked();
        // TODO: don't assume line height is constant
        let line_height = f64::from(self.line_height(0));
        let lines = (viewport.height() / line_height / 2.0).round() as usize;
        let distance = (lines as f64) * line_height;
        self.scroll_delta
            .set(Vec2::new(0.0, if down { distance } else { -distance }));
        let cmd = if down {
            MoveCommand::Down
        } else {
            MoveCommand::Up
        };
        let cmd = Command::Move(cmd);
        self.doc().run_command(self, &cmd, Some(lines), mods);
    }

    pub fn center_window(&self) {
        let viewport = self.viewport.get_untracked();
        // TODO: don't assume line height is constant
        let line_height = f64::from(self.line_height(0));
        let offset = self.cursor.with_untracked(|cursor| cursor.offset());
        let (line, _col) = self.offset_to_line_col(offset);

        let viewport_center = viewport.height() / 2.0;

        let current_line_position = line as f64 * line_height;

        let desired_top = current_line_position - viewport_center + (line_height / 2.0);

        let scroll_delta = desired_top - viewport.y0;

        self.scroll_delta.set(Vec2::new(0.0, scroll_delta));
    }

    pub fn top_of_window(&self, scroll_off: usize) {
        let viewport = self.viewport.get_untracked();
        // TODO: don't assume line height is constant
        let line_height = f64::from(self.line_height(0));
        let offset = self.cursor.with_untracked(|cursor| cursor.offset());
        let (line, _col) = self.offset_to_line_col(offset);

        let desired_top = (line.saturating_sub(scroll_off)) as f64 * line_height;

        let scroll_delta = desired_top - viewport.y0;

        self.scroll_delta.set(Vec2::new(0.0, scroll_delta));
    }

    pub fn bottom_of_window(&self, scroll_off: usize) {
        let viewport = self.viewport.get_untracked();
        // TODO: don't assume line height is constant
        let line_height = f64::from(self.line_height(0));
        let offset = self.cursor.with_untracked(|cursor| cursor.offset());
        let (line, _col) = self.offset_to_line_col(offset);

        let desired_bottom = (line + scroll_off + 1) as f64 * line_height - viewport.height();

        let scroll_delta = desired_bottom - viewport.y0;

        self.scroll_delta.set(Vec2::new(0.0, scroll_delta));
    }

    pub fn scroll(&self, top_shift: f64, down: bool, count: usize, mods: Modifiers) {
        let viewport = self.viewport.get_untracked();
        // TODO: don't assume line height is constant
        let line_height = f64::from(self.line_height(0));
        let diff = line_height * count as f64;
        let diff = if down { diff } else { -diff };

        let offset = self.cursor.with_untracked(|cursor| cursor.offset());
        let (line, _col) = self.offset_to_line_col(offset);
        let top = viewport.y0 + diff + top_shift;
        let bottom = viewport.y0 + diff + viewport.height();

        let new_line = if (line + 1) as f64 * line_height + line_height > bottom {
            let line = (bottom / line_height).floor() as usize;
            if line > 2 {
                line - 2
            } else {
                0
            }
        } else if line as f64 * line_height - line_height < top {
            let line = (top / line_height).ceil() as usize;
            line + 1
        } else {
            line
        };

        self.scroll_delta.set(Vec2::new(0.0, diff));

        let res = match new_line.cmp(&line) {
            Ordering::Greater => Some((MoveCommand::Down, new_line - line)),
            Ordering::Less => Some((MoveCommand::Up, line - new_line)),
            _ => None,
        };

        if let Some((cmd, count)) = res {
            let cmd = Command::Move(cmd);
            self.doc().run_command(self, &cmd, Some(count), mods);
        }
    }

    // === Information ===

    // pub fn phantom_text(&self, line: usize) -> PhantomTextLine {
    //     self.doc()
    //         .phantom_text(self.id(), &self.es.get_untracked(), line)
    // }

    pub fn line_height(&self, line: usize) -> f32 {
        self.doc().line_height(line)
    }

    // === Line Information ===

    // /// Iterate over the visual lines in the view, starting at the given line.
    // pub fn iter_vlines(
    //     &self,
    //     backwards: bool,
    //     start: VLine,
    // ) -> impl Iterator<Item = VLineInfo> + '_ {
    //     self.lines.iter_vlines(self.text_prov(), backwards, start)
    // }

    // /// Iterate over the visual lines in the view, starting at the given line and ending at the
    // /// given line. `start_line..end_line`
    // pub fn iter_vlines_over(
    //     &self,
    //     backwards: bool,
    //     start: VLine,
    //     end: VLine,
    // ) -> impl Iterator<Item = VLineInfo> + '_ {
    //     self.lines
    //         .iter_vlines_over(self.text_prov(), backwards, start, end)
    // }

    // /// Iterator over *relative* [`VLineInfo`]s, starting at the buffer line, `start_line`.
    // /// The `visual_line`s provided by this will start at 0 from your `start_line`.
    // /// This is preferable over `iter_lines` if you do not need to absolute visual line value.
    // pub fn iter_rvlines(
    //     &self,
    //     backwards: bool,
    //     start: RVLine,
    // ) -> impl Iterator<Item = VLineInfo<()>> + '_ {
    //     self.lines
    //         .iter_rvlines(self.text_prov().clone(), backwards, start)
    // }

    // /// Iterator over *relative* [`VLineInfo`]s, starting at the buffer line, `start_line` and
    // /// ending at `end_line`.
    // /// `start_line..end_line`
    // /// This is preferable over `iter_lines` if you do not need to absolute visual line value.
    // pub fn iter_rvlines_over(
    //     &self,
    //     backwards: bool,
    //     start: RVLine,
    //     end_line: usize,
    // ) -> impl Iterator<Item = VLineInfo<()>> + '_ {
    //     self.lines
    //         .iter_rvlines_over(self.text_prov(), backwards, start, end_line)
    // }

    // ==== Position Information ====

    pub fn first_rvline_info(&self) -> VLineInfo<VLine> {
        self.lines.with_untracked(|x| x.first_vline_info())
    }

    /// The number of lines in the document.
    pub fn num_lines(&self) -> usize {
        self.rope_text().num_lines()
    }

    /// The last allowed buffer line in the document.
    pub fn last_line(&self) -> usize {
        self.rope_text().last_line()
    }

    pub fn last_vline(&self) -> VLine {
        self.lines.with_untracked(|x| x.last_visual_line().into())
    }

    pub fn last_rvline(&self) -> RVLine {
        self.lines.with_untracked(|x| x.last_visual_line().into())
    }

    // pub fn last_rvline_info(&self) -> VLineInfo<()> {
    //     self.rvline_info(self.last_rvline())
    // }

    // ==== Line/Column Positioning ====

    /// Convert an offset into the buffer into a line and idx.  
    pub fn offset_to_line_col(&self, offset: usize) -> (usize, usize) {
        self.rope_text().offset_to_line_col(offset)
    }

    pub fn offset_of_line(&self, line: usize) -> usize {
        self.rope_text().offset_of_line(line)
    }

    pub fn offset_of_line_col(&self, line: usize, col: usize) -> usize {
        self.rope_text().offset_of_line_col(line, col)
    }

    /// Get the buffer line of an offset
    // pub fn line_of_offset(&self, offset: usize) -> usize {
    //     self.rope_text().line_of_offset(offset)
    // }

    /// Returns the offset into the buffer of the first non blank character on the given line.
    pub fn first_non_blank_character_on_line(&self, line: usize) -> usize {
        self.rope_text().first_non_blank_character_on_line(line)
    }

    pub fn line_end_col(&self, line: usize, caret: bool) -> usize {
        self.rope_text().line_end_col(line, caret)
    }

    pub fn select_word(&self, offset: usize) -> (usize, usize) {
        self.rope_text().select_word(offset)
    }

    /// `affinity` decides whether an offset at a soft line break is considered to be on the
    /// previous line or the next line.  
    /// If `affinity` is `CursorAffinity::Forward` and is at the very end of the wrapped line, then
    /// the offset is considered to be on the next line.
    pub fn vline_of_offset(&self, offset: usize, affinity: CursorAffinity) -> VLine {
        let (origin_line, offset_of_line) = self.doc.with_untracked(|x| {
            let text = x.text();
            let origin_line = text.line_of_offset(offset);
            let origin_line_start_offset = text.offset_of_line(origin_line);
            (origin_line, origin_line_start_offset)
        });
        let offset = offset - offset_of_line;
        self.lines.with_untracked(|x| x.visual_line_of_offset(origin_line, offset, affinity).0.vline)
    }

    // pub fn vline_of_line(&self, line: usize) -> VLine {
    //     self.lines.vline_of_line(self.text_prov(), line)
    // }

    // pub fn rvline_of_line(&self, line: usize) -> RVLine {
    //     self.lines.rvline_of_line(self.text_prov(), line)
    // }

    pub fn vline_of_rvline(&self, rvline: RVLine) -> VLine {
        self.lines.with_untracked(|x| x.visual_line_of_folded_line_and_sub_index(rvline.line, rvline.line_index).into())
    }

    // /// Get the nearest offset to the start of the visual line.
    // pub fn offset_of_vline(&self, vline: VLine) -> usize {
    //     self.lines.offset_of_vline(self.text_prov(), vline)
    // }

    // /// Get the visual line and column of the given offset.
    // /// The column is before phantom text is applied.
    // pub fn vline_col_of_offset(&self, offset: usize, affinity: CursorAffinity) -> (VLine, usize) {
    //     self.lines
    //         .vline_col_of_offset(self.text_prov(), offset, affinity)
    // }

    /// 该原始偏移字符所在的视觉行，以及在视觉行的偏移
    pub fn visual_line_of_offset(&self, offset: usize, affinity: CursorAffinity) -> (VLineInfo, usize, bool) {
        let (origin_line, offset_of_line) = self.doc.with_untracked(|x| {
            let text = x.text();
            let origin_line = text.line_of_offset(offset);
            let origin_line_start_offset = text.offset_of_line(origin_line);
            (origin_line, origin_line_start_offset)
        });
        let offset = offset - offset_of_line;
        self.lines.with_untracked(|x| x.visual_line_of_offset(origin_line, offset, affinity))
    }

    pub fn folded_line_of_offset(&self, offset: usize, _affinity: CursorAffinity) -> OriginFoldedLine {
        let line = self.visual_line_of_offset(offset, _affinity).0.rvline.line;
        self.lines.with_untracked(|x| x.folded_line_of_origin_line(line).clone())
    }

    // pub fn rvline_col_of_offset(&self, offset: usize, affinity: CursorAffinity) -> (RVLine, usize) {
    //     self.lines
    //         .rvline_col_of_offset(self.text_prov().clone(), offset, affinity)
    // }

    // pub fn offset_of_rvline(&self, rvline: RVLine) -> usize {
    //     self.lines.offset_of_rvline(self.text_prov(), rvline)
    // }

    // pub fn vline_info(&self, vline: VLine) -> VLineInfo {
    //     let vline = vline.min(self.last_vline());
    //     self.iter_vlines(false, vline).next().unwrap()
    // }

    // pub fn screen_rvline_info_of_offset(
    //     &self,
    //     offset: usize,
    //     affinity: CursorAffinity,
    // ) -> Option<VLineInfo<()>> {
    //     let rvline = self.visual_line_of_offset(offset, affinity);
    //     self.screen_lines.with_untracked(|screen_lines| {
    //         screen_lines
    //             .iter_vline_info()
    //             .find(|vline_info| vline_info.rvline == rvline)
    //     })
    // }

    // pub fn rvline_info(&self, rvline: RVLine) -> VLineInfo<()> {
    //     let rvline = rvline.min(self.last_rvline());
    //     self.iter_rvlines(false, rvline).next().unwrap()
    // }

    pub fn rvline_info_of_offset(&self, offset: usize, affinity: CursorAffinity) -> VLineInfo<VLine> {
        self.visual_line_of_offset(offset, affinity).0
    }

    /// Get the first column of the overall line of the visual line
    pub fn first_col<T: std::fmt::Debug>(&self, info: VLineInfo<T>) -> usize {
        info.first_col(self.text_prov())
    }

    /// Get the last column in the overall line of the visual line
    pub fn last_col<T: std::fmt::Debug>(&self, info: VLineInfo<T>, caret: bool) -> usize {
        info.last_col(self.text_prov(), caret)
    }

    // ==== Points of locations ====

    pub fn max_line_width(&self) -> f64 {
        self.lines.with_untracked(|x| x.max_width())
    }

    /// Returns the point into the text layout of the line at the given offset.
    /// `x` being the leading edge of the character, and `y` being the baseline.
    pub fn line_point_of_offset(&self, offset: usize, affinity: CursorAffinity) -> Point {
        let (line, col) = self.offset_to_line_col(offset);
        self.line_point_of_visual_line_col(line, col, affinity, false)
    }

    /// Returns the point into the text layout of the line at the given line and col.
    /// `x` being the leading edge of the character, and `y` being the baseline.  
    pub fn line_point_of_visual_line_col(
        &self,
        visual_line: usize,
        col: usize,
        affinity: CursorAffinity,
        _force_affinity: bool,
    ) -> Point {
        let text_layout = self.text_layout_of_visual_line(visual_line);
        // let index = if force_affinity {
        //     text_layout
        //         .phantom_text
        //         .col_after_force(visual_line, col, affinity == CursorAffinity::Forward)
        // } else {
        //     text_layout
        //         .phantom_text
        //         .col_after(visual_line, col, affinity == CursorAffinity::Forward)
        // };
        hit_position_aff(
            &text_layout.text,
            col,
            affinity == CursorAffinity::Backward,
        )
        .point
    }

    /// Get the (point above, point below) of a particular offset within the editor.
    pub fn points_of_offset(&self, offset: usize, affinity: CursorAffinity) -> (Point, Point) {
        let (line_info, line_offset, _) = self.visual_line_of_offset(offset, affinity);
        let line = line_info.vline.0;
        let line_height = f64::from(self.doc().line_height(line));

        let info = self.screen_lines.with_untracked(|sl| {
            sl.iter_line_info().find(|info| {
                info.vline_info.interval.start <= offset && offset <= info.vline_info.interval.end
            })
        });
        let Some(info) = info else {
            // TODO: We could do a smarter method where we get the approximate y position
            // because, for example, this spot could be folded away, and so it would be better to
            // supply the *nearest* position on the screen.
            return (Point::new(0.0, 0.0), Point::new(0.0, 0.0));
        };

        let y = info.vline_y;

        let x = self.line_point_of_visual_line_col(line, line_offset, affinity, false).x;

        (Point::new(x, y), Point::new(x, y + line_height))
    }

    /// Get the offset of a particular point within the editor.
    /// The boolean indicates whether the point is inside the text or not
    /// Points outside of vertical bounds will return the last line.
    /// Points outside of horizontal bounds will return the last column on the line.
    pub fn offset_of_point(&self, mode: Mode, point: Point, tracing: bool) -> (usize, bool) {
        let ((line, col), is_inside) = self.line_col_of_point(mode, point, tracing);

        let rs = (self.offset_of_line_col(line, col), is_inside);
        if tracing {
            tracing::info!("line={line} col={col} is_inside={is_inside} rs={rs:?}");
        }
        rs
    }

    /// 获取该坐标所在的视觉行和行偏离
    pub fn line_col_of_point_with_phantom(&self, point: Point) -> (usize, usize, Arc<TextLayoutLine>) {
        let line_height = f64::from(self.doc().line_height(0));
        let y = point.y.max(0.0);
        let visual_line = (y / line_height) as usize;
        let text_layout = self.text_layout_of_visual_line(visual_line);
        let hit_point = text_layout.text.hit_point(Point::new(point.x, y));
        (visual_line, hit_point.index, text_layout)
    }

    /// Get the (line, col) of a particular point within the editor.
    /// The boolean indicates whether the point is within the text bounds.
    /// Points outside of vertical bounds will return the last line.
    /// Points outside of horizontal bounds will return the last column on the line.
    pub fn line_col_of_point(&self, _mode: Mode, point: Point, _tracing: bool) -> ((usize, usize), bool) {
        // TODO: this assumes that line height is constant!
        let line_height = f64::from(self.doc().line_height(0));
        let info = if point.y <= 0.0 {
            self.first_rvline_info()
        } else {
            self.screen_lines
                .with_untracked(|sl| {
                    if let Some(info) = sl.iter_line_info().find(|info| {
                        info.vline_y <= point.y && info.vline_y + line_height >= point.y
                    }) {
                        info.vline_info
                    } else {
                        sl.info(*sl.lines.last().unwrap()).unwrap().vline_info
                    }
                })
        };
        // let info = info.unwrap_or_else(||{
        //     self.screen_lines
        //         .with_untracked(|sl| {
        //
        //         })
        //         .map(|info| info.vline_info)
        // });

        let rvline = info.rvline;
        let line = rvline.line;
        let text_layout = self.text_layout_of_visual_line(line);

        let y = text_layout.get_layout_y(rvline.line_index).unwrap_or(0.0);

        let hit_point = text_layout.text.hit_point(Point::new(point.x, y as f64));
        // We have to unapply the phantom text shifting in order to get back to the column in
        // the actual buffer
        let (line, col) = text_layout.phantom_text.origin_position_of_final_col(hit_point.index);
        // Ensure that the column doesn't end up out of bounds, so things like clicking on the far
        // right end will just go to the end of the line.
        // let max_col = self.line_end_col(line, mode != Mode::Normal);
        // let max_col = text_layout.text.line().text().len();
        // if line == 9 {
        //     tracing::info!("col={col} max_col={max_col} {hit_point:?} {} {} visual_line={}", self.rope_text().line_content(line), text_layout.text.line().text(), text_layout.phantom_text.visual_line)
        // }
        // let mut col = col.min(max_col);

        // TODO: we need to handle affinity. Clicking at end of a wrapped line should give it a
        // backwards affinity, while being at the start of the next line should be a forwards aff

        // TODO: this is a hack to get around text layouts not including spaces at the end of
        // wrapped lines, but we want to be able to click on them
        // if !hit_point.is_inside {
        //     // TODO(minor): this is probably wrong in some manners
        //     col = info.last_col(self.text_prov(), true);
        // }

        // let tab_width = self.style().tab_width(self.id(), line);
        // if self.style().atomic_soft_tabs(self.id(), line) && tab_width > 1 {
        //     col = snap_to_soft_tab_line_col(
        //         &self.text(),
        //         line,
        //         col,
        //         SnapDirection::Nearest,
        //         tab_width,
        //     );
        //     tracing::info!("snap_to_soft_tab_line_col col={col}");
        // }

        ((line, col), hit_point.is_inside)
    }

    // TODO: colposition probably has issues with wrapping?
    pub fn line_horiz_col(&self, line: usize, horiz: &ColPosition, caret: bool) -> (usize, usize) {
        match *horiz {
            ColPosition::Col(x) => {
                // TODO: won't this be incorrect with phantom text? Shouldn't this just use
                // line_col_of_point and get the col from that?
                let text_layout = self.text_layout_of_visual_line(line);
                let hit_point = text_layout.text.hit_point(Point::new(x, 0.0));
                let n = hit_point.index;
                text_layout.phantom_text.origin_position_of_final_col(n)
            }
            ColPosition::End => (line, self.line_end_col(line, caret)),
            ColPosition::Start => (line, 0),
            ColPosition::FirstNonBlank => (line, self.first_non_blank_character_on_line(line)),
        }
    }

    /// Advance to the right in the manner of the given mode.
    /// Get the column from a horizontal at a specific line index (in a text layout)
    pub fn rvline_horiz_col(
        &self,
        RVLine { line, line_index }: RVLine,
        horiz: &ColPosition,
        caret: bool,
    ) -> (usize, usize) {
        match *horiz {
            ColPosition::Col(x) => {
                let text_layout = self.text_layout_of_visual_line(line);
                let y_pos = text_layout
                    .text
                    .layout_runs()
                    .nth(line_index)
                    .map(|run| run.line_y)
                    .or_else(|| text_layout.text.layout_runs().last().map(|run| run.line_y))
                    .unwrap_or(0.0);
                let hit_point = text_layout.text.hit_point(Point::new(x, y_pos as f64));
                let n = hit_point.index;
                text_layout.phantom_text.origin_position_of_final_col(n)
            }
            // Otherwise it is the same as the other function
            _ => self.line_horiz_col(line, horiz, caret),
        }
    }

    /// Advance to the right in the manner of the given mode.  
    /// This is not the same as the [`Movement::Right`] command.
    pub fn move_right(&self, offset: usize, mode: Mode, count: usize) -> usize {
        self.rope_text().move_right(offset, mode, count)
    }

    /// Advance to the left in the manner of the given mode.
    /// This is not the same as the [`Movement::Left`] command.
    pub fn move_left(&self, offset: usize, mode: Mode, count: usize) -> usize {
        self.rope_text().move_left(offset, mode, count)
    }

    /// ~~视觉~~行的text_layout信息
    pub fn text_layout_of_visual_line(&self, line: usize) -> Arc<TextLayoutLine> {
        self.lines.with_untracked(|x| x.text_layout_of_visual_line(line))
    }

    // pub fn text_layout_trigger(&self, line: usize, trigger: bool) -> Arc<TextLayoutLine> {
    //     let cache_rev = self.doc().cache_rev().get_untracked();
    //     self.lines
    //         .get_init_text_layout(cache_rev, self.config_id(), self, line, trigger)
    // }

    // fn try_get_text_layout(&self, line: usize) -> Option<Arc<TextLayoutLine>> {
    //     let cache_rev = self.doc().cache_rev().get_untracked();
    //     self.lines
    //         .try_get_text_layout(cache_rev, self.config_id(), line)
    // }
}

impl std::fmt::Debug for Editor {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_tuple("Editor").field(&self.id).finish()
    }
}

// fn strip_suffix(line_content_original: &str) -> String {
//     if let Some(s) = line_content_original.strip_suffix("\r\n") {
//         format!("{s}  ")
//     } else if let Some(s) = line_content_original.strip_suffix('\n') {
//         format!("{s} ",)
//     } else {
//         line_content_original.to_string()
//     }
// }

fn push_strip_suffix(line_content_original: &str, rs: &mut String) {
    if let Some(s) = line_content_original.strip_suffix("\r\n") {
        rs.push_str(s);
        rs.push_str("  ");
        // format!("{s}  ")
    } else if let Some(s) = line_content_original.strip_suffix('\n') {
        rs.push_str(s);
        rs.push(' ');
    } else {
        rs.push_str(line_content_original);
    }
}

impl TextLayoutProvider for Editor {
    // TODO: should this just return a `Rope`?
    fn text(&self) -> Rope {
        Editor::text(self)
    }

    fn new_text_layout(&self, mut line: usize) -> Arc<TextLayoutLine> {
        // TODO: we could share text layouts between different editor views given some knowledge of
        // their wrapping
        let doc = self.doc();
        line = doc.visual_line_of_line(line);
        new_text_layout(doc, line)
    }

    /// 将列位置转换为合并前的位置，也就是原始文本的位置？意义？
    fn before_phantom_col(&self, line: usize, col: usize) -> (usize, usize) {
        self.new_text_layout(line)
            .phantom_text
            .origin_position_of_final_col(col)
        // self.doc()
        //     .before_phantom_col(self.id(), &self.es.get_untracked(), line, col)
    }

    // fn has_multiline_phantom(&self) -> bool {
    //     self.doc()
    //         .has_multiline_phantom(self.id(), &self.es.get_untracked())
    // }
}

pub struct EditorFontSizes {
    id: EditorId,
    style: ReadSignal<Rc<dyn Styling>>,
    doc: ReadSignal<Rc<dyn Document>>,
}
impl EditorFontSizes {
    fn font_size(&self, line: usize) -> usize {
        self.style
            .with_untracked(|style| style.font_size(line))
    }

    fn cache_id(&self) -> FontSizeCacheId {
        let mut hasher = DefaultHasher::new();

        // TODO: is this actually good enough for comparing cache state?
        // We could just have it return an arbitrary type that impl's Eq?
        self.style
            .with_untracked(|style| style.id().hash(&mut hasher));
        self.doc
            .with_untracked(|doc| doc.cache_rev().get_untracked().hash(&mut hasher));

        hasher.finish()
    }
}

/// Minimum width that we'll allow the view to be wrapped at.
const MIN_WRAPPED_WIDTH: f32 = 100.0;

/// Create various reactive effects to update the screen lines whenever relevant parts of the view,
/// doc, text layouts, viewport, etc. change.
/// This tries to be smart to a degree.
fn create_view_effects(cx: Scope, ed: &Editor) {
    // Cloning is fun.
    // let ed2 = ed.clone();
    let ed3 = ed.clone();
    let ed4 = ed.clone();

    // Reset cursor blinking whenever the cursor changes
    {
        let cursor_info = ed.cursor_info.clone();
        let cursor = ed.cursor;
        cx.create_effect(move |_| {
            cursor.track();
            cursor_info.reset();
        });
    }

    let update_screen_lines = |ed: &Editor| {
        // This function should not depend on the viewport signal directly.

        // This is wrapped in an update to make any updates-while-updating very obvious
        // which they wouldn't be if we computed and then `set`.
        ed.screen_lines.update(|screen_lines| {
            let new_screen_lines = ed.compute_screen_lines(screen_lines.base);

            *screen_lines = new_screen_lines;
        });
    };

    // Listen for layout events, currently only when a layout is created, and update screen
    // lines based on that
    // ed3.lines.with_untracked(|x| x.layout_event.listen_with(cx, move |val| {
    //     let ed = &ed2;
    //     // TODO: Move this logic onto screen lines somehow, perhaps just an auxiliary
    //     // function, to avoid getting confused about what is relevant where.
    //
    //     match val {
    //         LayoutEvent::CreatedLayout { line, .. } => {
    //             let sl = ed.screen_lines.get_untracked();
    //
    //             // Intelligently update screen lines, avoiding recalculation if possible
    //             let should_update = sl.on_created_layout(ed, line);
    //
    //             if should_update {
    //                 untrack(|| {
    //                     update_screen_lines(ed);
    //                 });
    //
    //                 // Ensure that it is created even after the base/viewport signals have been
    //                 // updated.
    //                 // But we have to trigger an event since it could alter the screenlines
    //                 // TODO: this has some risk for infinite looping if we're unlucky.
    //                 ed2.text_layout_trigger(line, true);
    //             }
    //         }
    //     }
    // }));

    // TODO: should we have some debouncing for editor width? Ideally we'll be fast enough to not
    // even need it, though we might not want to use a bunch of cpu whilst resizing anyway.

    let viewport_changed_trigger = cx.create_trigger();

    // Watch for changes to the viewport so that we can alter the wrapping
    // As well as updating the screen lines base
    cx.create_effect(move |_| {
        let ed = &ed3;

        let viewport = ed.viewport.get();

        let wrap = match ed.es.with(|s| s.wrap_method()) {
            WrapMethod::None => ResolvedWrap::None,
            WrapMethod::EditorWidth => {
                ResolvedWrap::Width((viewport.width() as f32).max(MIN_WRAPPED_WIDTH))
            }
            WrapMethod::WrapColumn { .. } => todo!(),
            WrapMethod::WrapWidth { width } => ResolvedWrap::Width(width),
        };

        ed.lines.update(|x| x.set_wrap(wrap, ed));
        // ed.lines.set_wrap(wrap, ed);

        // Update the base
        let base = ed.screen_lines.with_untracked(|sl| sl.base);

        // TODO: should this be a with or with_untracked?
        if viewport != base.with_untracked(|base| base.active_viewport) {
            batch(|| {
                base.update(|base| {
                    base.active_viewport = viewport;
                });
                // TODO: Can I get rid of this and just call update screen lines with an
                // untrack around it?
                viewport_changed_trigger.notify();
            });
        }
    });
    // Watch for when the viewport as changed in a relevant manner
    // and for anything that `update_screen_lines` tracks.
    cx.create_effect(move |_| {
        viewport_changed_trigger.track();

        update_screen_lines(&ed4);
    });
}

// pub fn normal_compute_screen_lines(
//     editor: &Editor,
//     base: RwSignal<ScreenLinesBase>,
// ) -> ScreenLines {
//     let lines = &editor.lines;
//     let style = editor.style.get();
//     // TODO: don't assume universal line height!
//     let line_height = style.line_height(editor.id(), 0);
//
//     let (y0, y1) = base.with_untracked(|base| (base.active_viewport.y0, base.active_viewport.y1));
//     // Get the start and end (visual) lines that are visible in the viewport
//     let min_vline = VLine((y0 / line_height as f64).floor() as usize);
//     let max_vline = VLine((y1 / line_height as f64).ceil() as usize);
//
//     let cache_rev = editor.doc.get().cache_rev().get();
//     editor.lines.check_cache_rev(cache_rev);
//
//     let min_info = editor.iter_vlines(false, min_vline).next();
//
//     let mut rvlines = Vec::new();
//     let mut info = HashMap::new();
//
//     let Some(min_info) = min_info else {
//         return ScreenLines {
//             lines: Rc::new(rvlines),
//             info: Rc::new(info),
//             diff_sections: None,
//             base,
//         };
//     };
//
//     // TODO: the original was min_line..max_line + 1, are we iterating too little now?
//     // the iterator is from min_vline..max_vline
//     let count = max_vline.get() - min_vline.get();
//     let iter = lines
//         .iter_rvlines_init(
//             editor.text_prov(),
//             cache_rev,
//             editor.config_id(),
//             min_info.rvline,
//             false,
//         )
//         .take(count);
//
//     for (i, vline_info) in iter.enumerate() {
//         rvlines.push(vline_info.rvline);
//
//         let line_height = f64::from(style.line_height(editor.id(), vline_info.rvline.line));
//
//         let y_idx = min_vline.get() + i;
//         let vline_y = y_idx as f64 * line_height;
//         let line_y = vline_y - vline_info.rvline.line_index as f64 * line_height;
//
//         // Add the information to make it cheap to get in the future.
//         // This y positions are shifted by the baseline y0
//         info.insert(
//             vline_info.rvline,
//             LineInfo {
//                 y: line_y - y0,
//                 vline_y: vline_y - y0,
//                 vline_info,
//             },
//         );
//     }
//
//     ScreenLines {
//         lines: Rc::new(rvlines),
//         info: Rc::new(info),
//         diff_sections: None,
//         base,
//     }
// }

// TODO: should we put `cursor` on this structure?
/// Cursor rendering information
#[derive(Clone)]
pub struct CursorInfo {
    pub hidden: RwSignal<bool>,

    pub blink_timer: RwSignal<TimerToken>,
    // TODO: should these just be rwsignals?
    pub should_blink: Rc<dyn Fn() -> bool + 'static>,
    pub blink_interval: Rc<dyn Fn() -> u64 + 'static>,
}
impl CursorInfo {
    pub fn new(cx: Scope) -> CursorInfo {
        CursorInfo {
            hidden: cx.create_rw_signal(false),

            blink_timer: cx.create_rw_signal(TimerToken::INVALID),
            should_blink: Rc::new(|| true),
            blink_interval: Rc::new(|| 500),
        }
    }

    pub fn blink(&self) {
        let info = self.clone();
        let blink_interval = (info.blink_interval)();
        if blink_interval > 0 && (info.should_blink)() {
            let blink_timer = info.blink_timer;
            let timer_token =
                exec_after(Duration::from_millis(blink_interval), move |timer_token| {
                    if info.blink_timer.try_get_untracked() == Some(timer_token) {
                        info.hidden.update(|hide| {
                            *hide = !*hide;
                        });
                        info.blink();
                    }
                });
            blink_timer.set(timer_token);
        }
    }

    pub fn reset(&self) {
        if self.hidden.get_untracked() {
            self.hidden.set(false);
        }

        self.blink_timer.set(TimerToken::INVALID);

        self.blink();
    }
}


fn new_text_layout(doc: Rc<dyn Document>, mut line: usize) -> Arc<TextLayoutLine> {
    // TODO: we could share text layouts between different editor views given some knowledge of
    // their wrapping
    let style = doc.clone();
    let es = doc.editor_style().get_untracked();
    let viewport = doc.viewport().get_untracked();

    let text = doc.rope_text();
    line = doc.visual_line_of_line(line);

    let mut line_content = String::new();
    // Get the line content with newline characters replaced with spaces
    // and the content without the newline characters
    // TODO: cache or add some way that text layout is created to auto insert the spaces instead
    // though we immediately combine with phantom text so that's a thing.
    let line_content_original = text.line_content(line);
    let mut font_system = FONT_SYSTEM.lock();
    push_strip_suffix(&line_content_original, &mut line_content);

    let family = style.font_family(line);
    let font_size = style.font_size(line);
    let attrs = Attrs::new()
        .color(es.ed_text_color())
        .family(&family)
        .font_size(font_size as f32)
        .line_height(LineHeightValue::Px(style.line_height(line)));

    let phantom_text = doc.phantom_text(&es, line);
    let mut collapsed_line_col = phantom_text.folded_line();
    let multi_styles: Vec<(usize, usize, Color, Attrs)> = style
        .line_styles(line)
        .into_iter()
        .map(|(start, end, color)| (start, end, color, attrs))
        .collect();

    let mut phantom_text = PhantomTextMultiLine::new(phantom_text);
    let mut attrs_list = AttrsList::new(attrs);
    for (start, end, color, attrs) in multi_styles.into_iter() {
        let (Some(start), Some(end)) = (phantom_text.col_at(start), phantom_text.col_at(end))
        else {
            continue;
        };
        attrs_list.add_span(start..end, attrs.color(color));
    }

    while let Some(collapsed_line) = collapsed_line_col.take() {
        push_strip_suffix(&text.line_content(collapsed_line), &mut line_content);

        let offset_col = phantom_text.final_text_len();
        let family = style.font_family(line);
        let font_size = style.font_size(line) as f32;
        let attrs = Attrs::new()
            .color(es.ed_text_color())
            .family(&family)
            .font_size(font_size)
            .line_height(LineHeightValue::Px(style.line_height(line)));
        // let (next_phantom_text, collapsed_line_content, styles, next_collapsed_line_col)
        //     = calcuate_line_text_and_style(collapsed_line, &next_line_content, style.clone(), edid, &es, doc.clone(), offset_col, attrs);

        let next_phantom_text = doc.phantom_text(&es, collapsed_line);
        collapsed_line_col = next_phantom_text.folded_line();
        let styles: Vec<(usize, usize, Color, Attrs)> = style
            .line_styles(collapsed_line)
            .into_iter()
            .map(|(start, end, color)| (start + offset_col, end + offset_col, color, attrs))
            .collect();

        for (start, end, color, attrs) in styles.into_iter() {
            let (Some(start), Some(end)) =
                (phantom_text.col_at(start), phantom_text.col_at(end))
            else {
                continue;
            };
            attrs_list.add_span(start..end, attrs.color(color));
        }
        phantom_text.merge(next_phantom_text);
    }
    let phantom_color = es.phantom_color();
    phantom_text.add_phantom_style(&mut attrs_list, attrs, font_size, phantom_color);

    // if line == 1 {
    //     tracing::info!("start");
    //     for (range, attr) in attrs_list.spans() {
    //         tracing::info!("{range:?} {attr:?}");
    //     }
    //     tracing::info!("");
    // }

    // tracing::info!("{line} {line_content}");
    // TODO: we could move tab width setting to be done by the document
    let final_line_content = phantom_text.final_line_content(&line_content);
    let mut text_layout = TextLayout::new_with_font_system(
        line,
        &final_line_content,
        attrs_list,
        &mut font_system,
    );
    drop(font_system);
    // text_layout.set_tab_width(style.tab_width(edid, line));

    // dbg!(self.editor_style.with(|s| s.wrap_method()));
    match es.wrap_method() {
        WrapMethod::None => {}
        WrapMethod::EditorWidth => {
            let width = viewport.width();
            text_layout.set_wrap(Wrap::WordOrGlyph);
            text_layout.set_size(width as f32, f32::MAX);
        }
        WrapMethod::WrapWidth { width } => {
            text_layout.set_wrap(Wrap::WordOrGlyph);
            text_layout.set_size(width, f32::MAX);
        }
        // TODO:
        WrapMethod::WrapColumn { .. } => {}
    }

    // let whitespaces = Self::new_whitespace_layout(
    //     &line_content_original,
    //     &text_layout,
    //     &phantom_text,
    //     es.render_whitespace(),
    // );
    // tracing::info!("line={line} {:?}", whitespaces);
    let indent_line = style.indent_line(line, &line_content_original);

    // let indent = if indent_line != line {
    //     // TODO: This creates the layout if it isn't already cached, but it doesn't cache the
    //     // result because the current method of managing the cache is not very smart.
    //     let layout = self.try_get_text_layout(indent_line).unwrap_or_else(|| {
    //         self.new_text_layout(
    //             indent_line,
    //             style.font_size(edid, indent_line),
    //             self.lines.wrap(),
    //         )
    //     });
    //     layout.indent + 1.0
    // } else {
    //     let offset = text.first_non_blank_character_on_line(indent_line);
    //     let (_, col) = text.offset_to_line_col(offset);
    //     text_layout.hit_position(col).point.x
    // };
    let offset = text.first_non_blank_character_on_line(indent_line);
    let (_, col) = text.offset_to_line_col(offset);
    let indent = text_layout.hit_position(col).point.x;

    let layout_line = TextLayoutLine {
        text: text_layout,
        extra_style: Vec::new(),
        whitespaces: None,
        indent,
        phantom_text,
    };
    // todo 下划线等？
    // let extra_style = style.apply_layout_styles(&layout_line.text, &layout_line.phantom_text, 0);
    //
    // layout_line.extra_style.clear();
    // layout_line.extra_style.extend(extra_style);

    Arc::new(layout_line)
}