//! Quill: the status item and its menu bar window (S-7, S-10). The menu bar
//! window sits next to the standard windows (D-17): the same Workspace, in
//! its own configuration (no sidebar at first, the S-8 New note field),
//! attached under the status item as a popover with an arrow (D-5). The Dock
//! icon shows only while a standard window is open (D-7).
use core_graphics::geometry::{CGPoint, CGRect, CGSize};
use gpui::*;
use objc::declare::ClassDecl;
use objc::runtime::{Class, Object, Sel, BOOL, NO, YES};
use objc::{class, msg_send, sel, sel_impl};
use raw_window_handle::{HasWindowHandle, RawWindowHandle};
use std::cell::RefCell;
use std::time::Duration;

use crate::app::Workspace;
use crate::settings::tr;

actions!(menu_bar, [ToggleMenuBarWindow]);

type Id = *mut Object;
const NIL: Id = std::ptr::null_mut();
/// The menu bar window's size (D-5: fixed).
const SIZE: (f64, f64) = (960., 640.);
const ARROW_H: f64 = 11.;
const ARROW_W: f64 = ARROW_H * 2.;
/// Space between the menu bar and the arrow's tip.
const GAP: f64 = 3.;

#[derive(Default)]
struct State { item: Option<Id>, target: Option<Id>, arrow: Option<Id>, app: Option<AsyncApp>, bar: Option<WindowHandle<Workspace>> }
thread_local! { static STATE: RefCell<State> = RefCell::new(State::default()); }

/// Scripted runs have no status item; QUILL_MENU_BAR=1 opens their window in
/// the menu bar configuration (no sidebar, the S-8 field).
pub fn scripted_bar() -> bool { crate::devtools::scripted() && std::env::var_os("QUILL_MENU_BAR").is_some() }

/// A new Workspace in the menu bar configuration (S-10 Req 3).
pub fn configure(w: &mut Workspace) { w.entry.menu_bar = true; w.sidebar_open = false; }

fn ns(s: &str) -> Id {
    let c = std::ffi::CString::new(s).unwrap_or_default();
    unsafe { msg_send![class!(NSString), stringWithUTF8String: c.as_ptr()] }
}
fn app() -> Id { unsafe { msg_send![class!(NSApplication), sharedApplication] } }
fn ns_window(window: &Window) -> Option<Id> {
    let RawWindowHandle::AppKit(h) = HasWindowHandle::window_handle(window).ok()?.as_raw() else { return None };
    let w: Id = unsafe { msg_send![h.ns_view.as_ptr() as Id, window] };
    (!w.is_null()).then_some(w)
}
fn bar_ns(cx: &mut App) -> Option<Id> {
    let handle = STATE.with(|s| s.borrow().bar)?;
    handle.update(cx, |_, window, _| ns_window(window)).ok().flatten()
}
fn is_bar(window: &Window) -> bool {
    STATE.with(|s| s.borrow().bar).is_some_and(|h| h.window_id() == window.window_handle().window_id())
}

/// Run `f` on the app from an AppKit callback.
fn dispatch(f: impl FnOnce(&mut App) + 'static) {
    let Some(cx) = STATE.with(|s| s.borrow().app.clone()) else { return };
    cx.foreground_executor().clone().spawn(async move { let _ = cx.update(f); }).detach();
}

extern "C" fn clicked(_: &Object, _: Sel, _: Id) {
    unsafe {
        let ev: Id = msg_send![app(), currentEvent];
        let (kind, flags): (u64, u64) = if ev.is_null() { (0, 0) } else { (msg_send![ev, type], msg_send![ev, modifierFlags]) };
        // NSEventTypeRightMouseUp, or Control-click.
        if kind == 4 || flags & (1 << 18) != 0 { show_menu(); } else { dispatch(toggle); }
    }
}
extern "C" fn menu_new(_: &Object, _: Sel, _: Id) { dispatch(new_note); }
extern "C" fn menu_window(_: &Object, _: Sel, _: Id) { dispatch(open_window); }
extern "C" fn menu_settings(_: &Object, _: Sel, _: Id) { dispatch(|cx| { show(cx); with_bar(cx, |_, window, cx| window.dispatch_action(Box::new(crate::app::OpenSettings), cx)); }); }
/// Devtools builds: the distributed notification `quill.devtools.status`
/// presses the status button (object "left"), opens its menu ("menu"), opens
/// or closes the standard windows ("window", "close") or quits ("quit"), for
/// checks run without accessibility access.
#[cfg(feature = "devtools")]
extern "C" fn devtools_press(_: &Object, _: Sel, note: Id) {
    unsafe {
        let what: Id = msg_send![note, object];
        let p: *const std::ffi::c_char = if what.is_null() { std::ptr::null() } else { msg_send![what, UTF8String] };
        let what = if p.is_null() { String::new() } else { std::ffi::CStr::from_ptr(p).to_string_lossy().into_owned() };
        match what.as_str() {
            "menu" => show_menu(),
            "window" => dispatch(open_window),
            "close" => dispatch(|cx| for h in standard_windows(cx) { let _ = h.update(cx, |_, window, _| window.remove_window()); }),
            "quit" => dispatch(|cx| cx.dispatch_action(&crate::app::Quit)),
            _ => { if let Some(item) = STATE.with(|s| s.borrow().item) { let b: Id = msg_send![item, button]; let _: () = msg_send![b, performClick: NIL]; } }
        }
    }
}
extern "C" fn menu_quit(_: &Object, _: Sel, _: Id) { dispatch(|cx| cx.dispatch_action(&crate::app::Quit)); }

fn target() -> Id {
    let class = Class::get("QuillStatusTarget").unwrap_or_else(|| {
        let mut decl = ClassDecl::new("QuillStatusTarget", class!(NSObject)).expect("status target class");
        unsafe {
            decl.add_method(sel!(quillClick:), clicked as extern "C" fn(&Object, Sel, Id));
            decl.add_method(sel!(quillNew:), menu_new as extern "C" fn(&Object, Sel, Id));
            decl.add_method(sel!(quillWindow:), menu_window as extern "C" fn(&Object, Sel, Id));
            decl.add_method(sel!(quillSettings:), menu_settings as extern "C" fn(&Object, Sel, Id));
            decl.add_method(sel!(quillQuit:), menu_quit as extern "C" fn(&Object, Sel, Id));
            #[cfg(feature = "devtools")]
            decl.add_method(sel!(quillDevtools:), devtools_press as extern "C" fn(&Object, Sel, Id));
        }
        decl.register()
    });
    unsafe { msg_send![class, new] }
}

fn show_menu() {
    let (Some(item), Some(target)) = STATE.with(|s| { let s = s.borrow(); (s.item, s.target) }) else { return };
    unsafe {
        let menu: Id = msg_send![class!(NSMenu), new];
        let rows: [(Option<&str>, Sel); 6] = [
            (Some(tr("新建笔记", "New Note")), sel!(quillNew:)),
            (Some(tr("打开窗口", "Open Window")), sel!(quillWindow:)),
            (None, sel!(quillNew:)),
            (Some(tr("设置…", "Settings…")), sel!(quillSettings:)),
            (None, sel!(quillNew:)),
            (Some(tr("退出 Quill", "Quit Quill")), sel!(quillQuit:)),
        ];
        for (title, action) in rows {
            let row: Id = match title {
                Some(t) => {
                    let row: Id = msg_send![class!(NSMenuItem), alloc];
                    let row: Id = msg_send![row, initWithTitle: ns(t) action: action keyEquivalent: ns("")];
                    let _: () = msg_send![row, setTarget: target];
                    let _: () = msg_send![row, autorelease];
                    row
                }
                None => msg_send![class!(NSMenuItem), separatorItem],
            };
            let _: () = msg_send![menu, addItem: row];
        }
        let _: () = msg_send![item, popUpStatusItemMenu: menu];
        let _: () = msg_send![menu, release];
    }
}

/// Add the status item. It stays while Quill runs (S-10 Req 2).
pub fn install(cx: &mut App) {
    cx.on_action(|_: &ToggleMenuBarWindow, cx| toggle(cx));
    let target = target();
    unsafe {
        let bar: Id = msg_send![class!(NSStatusBar), systemStatusBar];
        let item: Id = msg_send![bar, statusItemWithLength: -1.0f64];
        let _: () = msg_send![item, retain];
        let button: Id = msg_send![item, button];
        let png = include_bytes!("../assets/branding/menubar-icon.png");
        let data: Id = msg_send![class!(NSData), dataWithBytes: png.as_ptr() length: png.len()];
        let image: Id = msg_send![class!(NSImage), alloc];
        let image: Id = msg_send![image, initWithData: data];
        if !image.is_null() {
            let _: () = msg_send![image, setSize: CGSize::new(18., 18.)];
            let _: () = msg_send![image, setTemplate: YES];
            let _: () = msg_send![button, setImage: image];
            let _: () = msg_send![image, release];
        }
        let _: () = msg_send![button, setToolTip: ns("Quill")];
        let _: () = msg_send![button, setTarget: target];
        let _: () = msg_send![button, setAction: sel!(quillClick:)];
        // Left and right mouse up.
        let _: isize = msg_send![button, sendActionOn: (1u64 << 2) | (1u64 << 4)];
        #[cfg(feature = "devtools")]
        {
            let center: Id = msg_send![class!(NSDistributedNotificationCenter), defaultCenter];
            let _: () = msg_send![center, addObserver: target selector: sel!(quillDevtools:) name: ns("quill.devtools.status") object: NIL];
        }
        STATE.with(|s| { let mut s = s.borrow_mut(); s.item = Some(item); s.target = Some(target); s.app = Some(cx.to_async()); });
    }
    crate::quick_note::register_menubar(cx);
}

/// Quill was started as a login item (D-7 A): no window then, only the status item.
/// Valid while the launch event is handled (during `Application::run`'s callback).
pub fn launched_at_login() -> bool {
    unsafe {
        let m: Id = msg_send![class!(NSAppleEventManager), sharedAppleEventManager];
        let ev: Id = msg_send![m, currentAppleEvent];
        if ev.is_null() { return false; }
        // keyAEPropData 'prdt' holds keyAELaunchedAsLogInItem 'lgit'.
        let d: Id = msg_send![ev, paramDescriptorForKeyword: u32::from_be_bytes(*b"prdt")];
        if d.is_null() { return false; }
        let v: u32 = msg_send![d, enumCodeValue];
        v == u32::from_be_bytes(*b"lgit")
    }
}

/// The standard windows: Workspaces that are neither a quick note nor the menu bar window.
fn standard_windows(cx: &App) -> Vec<WindowHandle<Workspace>> {
    cx.windows().into_iter().filter_map(|w| w.downcast::<Workspace>())
        .filter(|h| h.read(cx).is_ok_and(|w| !w.quick_note && !w.entry.menu_bar)).collect()
}

/// The Dock icon follows the standard windows (D-7 B): shown while one is
/// open, hidden when only the menu bar window (or a quick note) is.
pub fn sync_policy(cx: &App) {
    if crate::devtools::scripted() || STATE.with(|s| s.borrow().item.is_none()) { return; }
    let regular = !standard_windows(cx).is_empty();
    // NSApplicationActivationPolicyRegular = 0, Accessory = 1.
    unsafe {
        let now: isize = msg_send![app(), activationPolicy];
        let want = if regular { 0isize } else { 1isize };
        if now != want { let _: BOOL = msg_send![app(), setActivationPolicy: want]; }
    }
}

/// "Open Window", a Dock click or a launch from the Finder: bring a standard
/// window forward, or open one (S-10 Req 4).
pub fn open_window(cx: &mut App) {
    match standard_windows(cx).first() {
        Some(h) => { let _ = h.update(cx, |_, window, _| window.activate_window()); }
        None => crate::open_main_window(false, cx),
    }
    sync_policy(cx);
    cx.activate(true);
}

fn with_bar(cx: &mut App, f: impl FnOnce(&mut Workspace, &mut Window, &mut Context<Workspace>)) {
    if let Some(handle) = STATE.with(|s| s.borrow().bar) { let _ = handle.update(cx, f); }
}

/// The menu bar window, opened (hidden) the first time it is needed.
fn ensure_bar(cx: &mut App) -> Option<Id> {
    if let Some(w) = bar_ns(cx) { return Some(w); }
    let handle = cx.open_window(
        WindowOptions {
            window_bounds: Some(WindowBounds::Windowed(Bounds::centered(None, size(px(SIZE.0 as f32), px(SIZE.1 as f32)), cx))),
            window_background: crate::theme::window_background(),
            app_id: Some(crate::APP_ID.into()),
            titlebar: Some(TitlebarOptions { title: Some("Quill".into()), appears_transparent: true, traffic_light_position: None }),
            show: false,
            is_movable: false,
            is_resizable: false,
            is_minimizable: false,
            ..Default::default()
        },
        |window, cx| cx.new(|cx| { let mut w = Workspace::new(window, cx); configure(&mut w); w }),
    ).ok()?;
    STATE.with(|s| s.borrow_mut().bar = Some(handle));
    let w = bar_ns(cx)?;
    style(w);
    Some(w)
}

/// Menu bar window chrome: no traffic lights, follows the user across Spaces.
fn style(w: Id) {
    unsafe {
        for b in 0u64..3 {
            let button: Id = msg_send![w, standardWindowButton: b];
            if !button.is_null() { let _: () = msg_send![button, setHidden: YES]; }
        }
        // MoveToActiveSpace | FullScreenAuxiliary.
        let _: () = msg_send![w, setCollectionBehavior: (1u64 << 1) | (1u64 << 8)];
    }
}

fn visible(w: Id) -> bool { unsafe { let v: BOOL = msg_send![w, isVisible]; v == YES } }
fn app_active() -> bool { unsafe { let v: BOOL = msg_send![app(), isActive]; v == YES } }
fn key(w: Id) -> bool { unsafe { let v: BOOL = msg_send![w, isKeyWindow]; v == YES } }

/// Status item click or the global shortcut: show or hide the menu bar window.
pub fn toggle(cx: &mut App) {
    match bar_ns(cx) {
        Some(w) if visible(w) && app_active() && key(w) => hide(cx),
        _ => show(cx),
    }
}

/// Run AppKit window changes after the current app update returns: GPUI's
/// resize callback needs the app, and a resize made while it is borrowed is lost.
fn native(cx: &App, f: impl FnOnce() + 'static) { cx.foreground_executor().spawn(async move { f() }).detach(); }

/// Show the menu bar window under the status item.
pub fn show(cx: &mut App) {
    let Some(w) = ensure_bar(cx) else { return };
    let color: Rgba = crate::theme::Theme::bg_body().into();
    native(cx, move || {
        unsafe {
            let _: () = msg_send![w, makeKeyAndOrderFront: NIL];
            let _: () = msg_send![app(), activateIgnoringOtherApps: YES];
        }
        // Placed once on screen: AppKit moves a window that is ordered in after its frame was set.
        place(w, color);
    });
}

/// Hide the menu bar window, saving the open note first.
pub fn hide(cx: &mut App) {
    with_bar(cx, |w, _, cx| { w.flush(cx); });
    let Some(w) = bar_ns(cx) else { return };
    unsafe {
        if let Some(arrow) = STATE.with(|s| s.borrow().arrow) {
            let _: () = msg_send![w, removeChildWindow: arrow];
            let _: () = msg_send![arrow, orderOut: NIL];
        }
        let _: () = msg_send![w, orderOut: NIL];
    }
}

/// Put the window under the status item, on the display that holds it, with
/// the arrow pointing at the icon.
fn place(w: Id, color: Rgba) {
    let Some(item) = STATE.with(|s| s.borrow().item) else { return };
    unsafe {
        let button: Id = msg_send![item, button];
        let bar: Id = msg_send![button, window];
        if bar.is_null() { return; }
        // The button's own rect on screen (the status bar window can be wider).
        let bounds: CGRect = msg_send![button, bounds];
        let in_window: CGRect = msg_send![button, convertRect: bounds toView: NIL];
        let icon: CGRect = msg_send![bar, convertRectToScreen: in_window];
        let mut screen: Id = msg_send![bar, screen];
        if screen.is_null() { screen = msg_send![class!(NSScreen), mainScreen]; }
        let vis: CGRect = msg_send![screen, visibleFrame];
        let full: CGRect = msg_send![screen, frame];
        let mid = icon.origin.x + icon.size.width / 2.;
        // Windows stay out of the menu bar, so the arrow's tip sits at the bar's bottom edge
        // (with the bar auto-hidden the visible frame reaches the screen top).
        let top = (icon.origin.y - GAP).min(vis.origin.y + vis.size.height).min(full.origin.y + full.size.height) - ARROW_H;
        let width = SIZE.0.min(vis.size.width - 16.);
        let height = SIZE.1.min(top - vis.origin.y - 8.);
        let x = (mid - width / 2.).clamp(vis.origin.x + 8., vis.origin.x + vis.size.width - width - 8.);
        let frame = CGRect::new(&CGPoint::new(x, top - height), &CGSize::new(width, height));
        let _: () = msg_send![w, setFrame: frame display: YES];
        let arrow = STATE.with(|s| s.borrow().arrow).unwrap_or_else(|| { let a = make_arrow(); STATE.with(|s| s.borrow_mut().arrow = Some(a)); a });
        paint_arrow(arrow, color);
        let arrow_x = (mid - ARROW_W / 2.).clamp(x + 14., x + width - ARROW_W - 14.);
        let _: () = msg_send![arrow, setFrame: CGRect::new(&CGPoint::new(arrow_x, top), &CGSize::new(ARROW_W, ARROW_H)) display: YES];
        let _: () = msg_send![w, addChildWindow: arrow ordered: 1isize];
    }
}

#[repr(C)]
#[derive(Clone, Copy)]
struct Affine { a: f64, b: f64, c: f64, d: f64, tx: f64, ty: f64 }

/// The arrow: a borderless child window holding a square turned 45°, cut
/// off at the window's bottom edge so only the top half (a triangle) shows.
fn make_arrow() -> Id {
    unsafe {
        let rect = CGRect::new(&CGPoint::new(0., 0.), &CGSize::new(ARROW_W, ARROW_H));
        let w: Id = msg_send![class!(NSWindow), alloc];
        let w: Id = msg_send![w, initWithContentRect: rect styleMask: 0u64 backing: 2u64 defer: NO];
        let _: () = msg_send![w, setReleasedWhenClosed: NO];
        let _: () = msg_send![w, setOpaque: NO];
        let clear: Id = msg_send![class!(NSColor), clearColor];
        let _: () = msg_send![w, setBackgroundColor: clear];
        let _: () = msg_send![w, setHasShadow: NO];
        let _: () = msg_send![w, setIgnoresMouseEvents: YES];
        let view: Id = msg_send![w, contentView];
        let _: () = msg_send![view, setWantsLayer: YES];
        let root: Id = msg_send![view, layer];
        let _: () = msg_send![root, setMasksToBounds: YES];
        let square: Id = msg_send![class!(CALayer), layer];
        let side = ARROW_H * std::f64::consts::SQRT_2;
        let _: () = msg_send![square, setBounds: CGRect::new(&CGPoint::new(0., 0.), &CGSize::new(side, side))];
        let _: () = msg_send![square, setPosition: CGPoint::new(ARROW_W / 2., 0.)];
        let r = std::f64::consts::FRAC_1_SQRT_2;
        let _: () = msg_send![square, setAffineTransform: Affine { a: r, b: r, c: -r, d: r, tx: 0., ty: 0. }];
        let _: () = msg_send![root, addSublayer: square];
        w
    }
}

/// The arrow takes the color of the window's top bar.
fn paint_arrow(arrow: Id, c: Rgba) {
    unsafe {
        let view: Id = msg_send![arrow, contentView];
        let root: Id = msg_send![view, layer];
        let subs: Id = msg_send![root, sublayers];
        let square: Id = msg_send![subs, firstObject];
        if square.is_null() { return; }
        let color: Id = msg_send![class!(NSColor), colorWithSRGBRed: c.r as f64 green: c.g as f64 blue: c.b as f64 alpha: 1.0f64];
        let cg: *const std::ffi::c_void = msg_send![color, CGColor];
        let _: () = msg_send![square, setBackgroundColor: cg];
    }
}

/// A window lost focus: the menu bar window hides once Quill is no longer
/// the active app or another Quill window has the focus (D-5). Quill's own
/// dialogs (open and save panels, alerts) keep it.
pub fn deactivated(window: &Window, cx: &mut App) {
    if crate::devtools::scripted() || !is_bar(window) { return; }
    cx.spawn(async move |cx| {
        cx.background_executor().timer(Duration::from_millis(60)).await;
        let _ = cx.update(|cx| if bar_ns(cx).is_some_and(visible) && (!app_active() || other_window_key(cx)) { hide(cx); });
    }).detach();
}
/// Another Quill window (a standard window or a quick note) is the key window.
fn other_window_key(cx: &mut App) -> bool {
    let key: Id = unsafe { msg_send![app(), keyWindow] };
    if key.is_null() { return false; }
    let bar = STATE.with(|s| s.borrow().bar).map(|h| h.window_id());
    cx.windows().into_iter().filter(|w| Some(w.window_id()) != bar)
        .any(|w| w.update(cx, |_, window, _| ns_window(window) == Some(key)).unwrap_or(false))
}
/// Esc with nothing else to close, or Cmd-W, in the menu bar window: hide instead of closing.
pub fn hide_main(window: &Window, cx: &mut App) -> bool {
    if crate::devtools::scripted() || !is_bar(window) { return false; }
    cx.defer(hide);
    true
}

/// A new note from the status item menu: show the menu bar window and open a blank note.
fn new_note(cx: &mut App) {
    show(cx);
    with_bar(cx, |_, window, cx| window.dispatch_action(Box::new(crate::app::NewDoc), cx));
}

#[link(name = "ServiceManagement", kind = "framework")]
unsafe extern "C" {}

fn login_service() -> Option<Id> {
    let class = Class::get("SMAppService")?;
    let s: Id = unsafe { msg_send![class, mainAppService] };
    (!s.is_null()).then_some(s)
}
/// Settings > General > Launch at login (D-7 A). The system's login items are the record.
pub fn launch_at_login() -> bool {
    // SMAppServiceStatusEnabled = 1.
    login_service().is_some_and(|s| unsafe { let st: isize = msg_send![s, status]; st == 1 })
}
pub fn set_launch_at_login(on: bool) -> Result<(), String> {
    let s = login_service().ok_or_else(|| tr("这个 macOS 版本不支持", "Not supported on this macOS").to_string())?;
    unsafe {
        let mut err: Id = NIL;
        let ok: BOOL = if on { msg_send![s, registerAndReturnError: &mut err] } else { msg_send![s, unregisterAndReturnError: &mut err] };
        if ok == YES { return Ok(()); }
        if err.is_null() { return Err(tr("登录项设置失败", "Could not change the login item").into()); }
        let d: Id = msg_send![err, localizedDescription];
        let p: *const std::ffi::c_char = msg_send![d, UTF8String];
        Err(std::ffi::CStr::from_ptr(p).to_string_lossy().into_owned())
    }
}
