//! Quill: the 新建笔记 field on top of 主页 when the main window is shown
//! from the menu bar (S-8). Focusing it grows the field into the new note's
//! editor while 主页's content slides out of the bottom; Esc on a blank note
//! reverses it. The note is written to disk only once it has content.
use gpui::*;
use std::time::Duration;

use crate::app::{View, Workspace};
use crate::theme::{self, Theme};

#[derive(Default)]
pub struct NewEntry {
    /// The window is shown from the menu bar (S-7). Only then 主页 has the field.
    pub menu_bar: bool,
    /// A note started from the field: it has an id but no file until it has content.
    pub pending: Option<String>,
    anim: Option<Anim>,
    seq: usize,
    done: Option<Task<()>>,
}

#[derive(Clone, Copy)]
struct Anim { expanding: bool, seq: usize, reduce: bool }

impl NewEntry {
    pub fn is_pending(&self, id: &str) -> bool { self.pending.as_deref() == Some(id) }
    pub fn animating(&self) -> bool { self.anim.is_some() }
}

/// macOS 减弱动态效果: a cross-fade instead of the slide.
fn reduce_motion() -> bool {
    // Scripted runs can't change the system setting; QUILL_REDUCE_MOTION=1 stands in for it.
    if crate::devtools::scripted() && std::env::var_os("QUILL_REDUCE_MOTION").is_some() { return true; }
    unsafe {
        use objc::runtime::{BOOL, Object, YES};
        use objc::{class, msg_send, sel, sel_impl};
        let ws: *mut Object = msg_send![class!(NSWorkspace), sharedWorkspace];
        let r: BOOL = msg_send![ws, accessibilityDisplayShouldReduceMotion];
        r == YES
    }
}

impl Workspace {
    /// The field was focused: open a blank note in it and grow it into the editor.
    pub fn start_new_entry(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        if self.entry.animating() || !self.entry.menu_bar { return; }
        let id = uuid::Uuid::new_v4().to_string();
        self.entry.pending = Some(id.clone());
        if !self.try_open(&id, window, cx) { self.entry.pending = None; return; }
        // What you type is the body's first line (D-9), not the title.
        if let Some(ed) = &self.editor {
            ed.update(cx, |e, cx| {
                use crate::editor::core::{Pos, Selection};
                if let Some(b) = e.core.doc.blocks.first() { e.core.sel = Selection::caret(Pos::new(b.id, 0)); }
                cx.notify();
            });
        }
        // Stay on 主页 while the field grows; the editor already has the caret.
        self.view = View::Home;
        self.run_entry_anim(true, window, cx);
    }

    /// Esc on a blank note from the field: shrink it back into the field.
    /// False when Esc is not ours (the note has content, or it is any other note).
    pub fn leave_new_entry(&mut self, window: &mut Window, cx: &mut Context<Self>) -> bool {
        let Some(id) = self.current.clone() else { return false };
        if self.view != View::Doc || !self.entry.is_pending(&id) || self.entry.animating() { return false; }
        if !self.editor.as_ref().is_some_and(|e| e.read(cx).core.doc.is_blank()) { return false; }
        self.view = View::Home;
        self.run_entry_anim(false, window, cx);
        true
    }

    fn run_entry_anim(&mut self, expanding: bool, window: &mut Window, cx: &mut Context<Self>) {
        self.entry.seq += 1;
        let reduce = reduce_motion();
        self.entry.anim = Some(Anim { expanding, seq: self.entry.seq, reduce });
        let ms = if reduce { theme::ENTRY_FADE_MS } else { theme::ENTRY_SLIDE_MS };
        let seq = self.entry.seq;
        self.entry.done = Some(cx.spawn_in(window, async move |this, cx| {
            cx.background_executor().timer(Duration::from_millis(ms)).await;
            let _ = this.update_in(cx, |w, window, cx| w.finish_entry_anim(seq, window, cx));
        }));
        cx.notify();
    }

    fn finish_entry_anim(&mut self, seq: usize, window: &mut Window, cx: &mut Context<Self>) {
        let Some(anim) = self.entry.anim.filter(|a| a.seq == seq) else { return };
        self.entry.anim = None;
        if anim.expanding {
            self.view = View::Doc;
            if let Some(ed) = &self.editor { ed.read(cx).focus(window); }
        } else {
            // A blank note never reached the disk: drop it.
            self.editor = None;
            self.editor_sub = None;
            self.current = None;
            self.entry.pending = None;
            window.focus(&self.focus);
        }
        cx.notify();
    }

    /// The pending note was saved for the first time: it is a normal note now.
    pub(crate) fn entry_saved(&mut self, id: &str) {
        if self.entry.is_pending(id) {
            self.entry.pending = None;
            self.entries = self.store.list();
            self.sort();
        }
    }

    /// 主页's content below the header: the field, then the tabs and documents.
    /// While the field grows the content slides down by the same fraction.
    pub(crate) fn render_entry_home(&mut self, rest: AnyElement, cx: &mut Context<Self>) -> AnyElement {
        let field_top = 12.;
        let below = field_top + theme::ENTRY_FIELD_H;
        let anim = self.entry.anim;
        let editor = self.editor.clone().filter(|_| anim.is_some());
        let field = div()
            .id("new-entry-field")
            .absolute()
            .top(px(field_top))
            .left(px(24.))
            .right(px(24.))
            .h(px(theme::ENTRY_FIELD_H))
            .rounded(px(9.))
            .border_1()
            .border_color(Theme::line_border())
            .bg(Theme::fill_hover())
            .overflow_hidden()
            .cursor_text()
            .child(
                div().h(px(theme::ENTRY_FIELD_H - 2.)).px(px(12.)).flex().items_center().gap(px(10.))
                    .child(div().text_size(px(17.)).text_color(Theme::text_placeholder()).child("+"))
                    .child(div().text_size(px(13.)).text_color(Theme::text_placeholder()).child(crate::settings::tr("新建笔记", "New note")))
                    .child(div().flex_1())
                    .child(div().text_size(px(11.)).text_color(Theme::text_disabled()).child(crate::settings::tr("点击开始书写", "Click to write"))),
            )
            .on_click(cx.listener(|w, _, window, cx| w.start_new_entry(window, cx)));
        let rest = div().absolute().left_0().right_0().top_0().h_full().pt(px(below)).flex().flex_col().child(rest);
        let Some(anim) = anim else {
            return div().relative().flex_1().min_h_0().overflow_hidden().child(rest).child(field).into_any_element();
        };
        let ms = if anim.reduce { theme::ENTRY_FADE_MS } else { theme::ENTRY_SLIDE_MS };
        let ease = |t: f32| 1. - (1. - t).powi(3);
        let (expanding, reduce) = (anim.expanding, anim.reduce);
        // 0 = the field at rest, 1 = the field filling the page.
        let at = move |t: f32| { let k = ease(t); if expanding { k } else { 1. - k } };
        let grown = div()
            .absolute().top(px(field_top)).left(px(24.)).right(px(24.)).bottom(px(field_top))
            .rounded(px(9.)).border_1().border_color(Theme::primary()).bg(Theme::bg_body()).overflow_hidden()
            .children(editor);
        div()
            .relative().flex_1().min_h_0().overflow_hidden()
            .child(rest.with_animation(("entry-rest", anim.seq), Animation::new(Duration::from_millis(ms)), move |d, t| {
                let k = at(t);
                if reduce { d.opacity(1. - k) } else { d.top(relative(k)).opacity(1. - k * 0.8) }
            }))
            .child(grown.with_animation(("entry-grow", anim.seq), Animation::new(Duration::from_millis(ms)), move |d, t| {
                let k = at(t);
                // The field's bottom edge travels from its resting height to the page's bottom.
                if reduce { d.opacity(k) } else { d.bottom(relative(1. - k)).min_h(px(theme::ENTRY_FIELD_H)).opacity(0.4 + 0.6 * k) }
            }))
            .into_any_element()
    }
}
