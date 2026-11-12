//! `App::should_close` on the headless desktop: a refused close leaves the window open.

use super::*;

/// What the window's close hooks ran, in order.
static HOOKS: std::sync::Mutex<Vec<&'static str>> = std::sync::Mutex::new(Vec::new());

/// A `keep_running` window any key asks to close. It refuses once, turning green as an app
/// would show its "Save changes?", and lets the next close through.
fn asks_once() -> crate::AnyWindowSpec {
    let spec = AppSpec {
        on_key: Some(|_: &mut Pair, _: &crate::KeyEvent, tx: &mut crate::Tx| { tx.close_window(); true }),
        ..AppSpec::new("asks", (1.0, 1.0), || Pair { name: "", color: crate::Color::TRANSPARENT, other: None, act: |_, _| {} },
            |p, _| crate::el().w_full().h_full().fill(p.color))
    };
    let app = crate::App::new(spec).init(|| Pair { name: "a", color: RED, other: None, act: |_, _| {} }).keep_running()
        .should_close(|p, tx| {
            HOOKS.lock().unwrap().push("asked");
            let asked_before = p.color == GREEN;
            p.color = GREEN;
            tx.instant();
            asked_before
        })
        .on_close(|_, _| HOOKS.lock().unwrap().push("closed"));
    crate::WindowSpec::from_app("a", app).options(left()).boxed()
}

#[test]
fn desktop_window_that_refuses_a_close_stays_open_and_shows_what_its_hook_changed() {
    let _turn = HEADLESS.lock().unwrap_or_else(|e| e.into_inner());
    HOOKS.lock().unwrap().clear();
    let (png, run) = desktop_case("refuse", r#"[{"t": 0.1, "type": "key", "name": "Enter"}]"#, 0.1, &[], || vec![asks_once()]);
    assert_eq!(*HOOKS.lock().unwrap(), ["asked"], "on_close never ran");
    assert_eq!(focus_of(run, 0.1, "a"), (true, true), "open, with the keyboard it had");
    let p = png.get_pixel(70, 50).0;
    assert!(p[1] > 245 && p[0] < 10, "the refusal's change shows in that frame: {p:?}");
}

#[test]
fn desktop_window_closes_once_should_close_allows_it() {
    let _turn = HEADLESS.lock().unwrap_or_else(|e| e.into_inner());
    HOOKS.lock().unwrap().clear();
    let script = r#"[{"t": 0.1, "type": "key", "name": "Enter"}, {"t": 0.2, "type": "key", "name": "Enter"}]"#;
    let (png, run) = desktop_case("allow", script, 0.2, &[], || vec![asks_once()]);
    assert_eq!(*HOOKS.lock().unwrap(), ["asked", "asked", "closed"]);
    assert_eq!((focus_of(run, 0.1, "a"), focus_of(run, 0.2, "a")), ((true, true), (false, false)));
    assert_eq!(png.get_pixel(70, 50).0, [128, 128, 128, 255], "the window is gone from the frame");
}
