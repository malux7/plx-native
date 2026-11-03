//! What the page image of a Home → Library push contains: the product's path, not a frame dump's.

use super::*;
#[allow(unused_imports)]
use super::test_support::*;

/// One row of the trace: what the frame's draw did and what the Library would put on screen.
struct Row { top: &'static str, captured: bool, image: Option<f32>, content: Option<(f32, f32, bool, i64, usize)> }

fn library_content(d: &Dispatcher<AppHost>) -> Option<(f32, f32, bool, i64, usize)> {
    let entry = d.nav.top_page()?;
    let page = entry.inst.as_ref()?.screen.as_any()?.downcast_ref::<plx_screens::library::LibraryScreen>()?;
    Some(page.probe_content())
}

#[derive(Clone, Copy, PartialEq, Debug)]
enum Case { First, Revisit, OtherKind }

fn push_trace(case: Case) -> Vec<Row> {
    let revisit = case == Case::Revisit;
    struct Cleanup;
    impl Drop for Cleanup { fn drop(&mut self) { plx_plex::plex::reset_servers_for_test(); } }
    let _cleanup = Cleanup;
    let _held = ImageHeld;
    let session = plx_plex::plex::session::TempSession::new("library-push-capture");
    session.watching("u-library-push-capture");
    plx_plex::plex::reset_servers_for_test();
    let sid = plx_plex::plex::register_for_test("push-own", "127.0.0.1", 9, "synthetic", "fixture");
    let shared = plx_plex::plex::register_for_test("push-shared", "127.0.0.1", 10, "synthetic", "fixture");
    plx_plex::plex::set_current(sid);
    let mut d = Dispatcher::<AppHost>::with_transition(Box::new(plx_ui::containers::transition::PageDip::new()));
    let mut rig = Bridge::for_test(|| 0);
    // WARM: the section table, the listing and its shelves are all in the store before the press.
    rig.stores.browse.borrow_mut().seed_registered_table_for_test([sid, shared]);
    rig.refresh_browse_directory();
    rig.browse_run(plx_data::stores::browse::BrowseCmd::SetCur(0));
    rig.stores.browse.borrow_mut().seed_items_for_test(120);
    rig.stores.browse.borrow_mut().seed_shelves_for_test(0, &[], 4);
    if case == Case::OtherKind {
        // TV Shows is warm too (60 items) and is where the store points: the reader's last
        // library. They visit it, go Home, and press Movies.
        rig.browse_run(plx_data::stores::browse::BrowseCmd::SetCur(1));
        rig.stores.browse.borrow_mut().seed_items_for_test(60);
        rig.stores.browse.borrow_mut().seed_shelves_for_test(1, &[], 4);
    }
    let tab = HomeTab::Movies;
    // The page pass is GL, which a host test has none of, so the frame's draw is not run: its
    // image decision is (`DipImage`; quiescence is layout motion only: no upload queue here).
    let mut dip = DipImage::default();
    let mut ms = 0u32;
    let mut step = |d: &mut Dispatcher<AppHost>, rig: &mut Bridge, rows: Option<&mut Vec<Row>>| {
        ms += 16;
        plx_machine::idle::frame_begin(0.016);
        super::frame(d, rig, Tick { ms, dt_us: 16_000 }, vec![]);
        use plx_ui::containers::transition::PagePaint;
        let Some(paint) = dip.plan(d, ms) else { return };
        let transition = &d.nav.tabs.stack.transition;
        if let Some(rows) = rows {
            rows.push(Row {
                top: d.top_screen().map_or("", |s| s.name()),
                captured: paint.captures_page(),
                image: match paint { PagePaint::Capture => Some(transition.page_alpha()), other => other.frozen_alpha() },
                content: library_content(d) });
        }
    };
    super::show_page(&mut d, AppArg::Home);
    for _ in 0..60 { step(&mut d, &mut rig, None); }
    if revisit || case == Case::OtherKind {
        nav_tab(&mut d, &mut rig, if revisit { tab } else { HomeTab::Shows }, None, None);
        for _ in 0..90 { step(&mut d, &mut rig, None); }
        assert!(d.top_arg() == Some(&AppArg::Library));
        nav_tab(&mut d, &mut rig, HomeTab::Home, None, None);
        for _ in 0..90 { step(&mut d, &mut rig, None); }
        assert!(d.top_arg() == Some(&AppArg::Home));
    }
    let mut rows = Vec::new();
    nav_tab(&mut d, &mut rig, tab, None, None);
    for _ in 0..22 {
        step(&mut d, &mut rig, Some(&mut rows));
    }
    assert!(d.top_arg() == Some(&AppArg::Library), "premise: the push committed");
    rows
}

/// **The image a pushed Library is shown as carries the library that was pressed.** A dip draws
/// its destination as ONE image captured at the floor (`PageImage::step`) and holds it through
/// the In half and on until the page is at rest, so what the Library can draw on the frame it is
/// mounted is what the reader watches arrive. With the pressed library's listing already in the
/// store that has to be its tiles: an image without them is an empty page (a still spinner)
/// fading in for the whole dip, then the finished grid in one frame.
#[test]
fn a_warm_library_push_captures_the_pressed_library_with_its_tiles() {
    let _guard = plx_base::testlock::serial();
    let mut bad = Vec::new();
    for case in [Case::First, Case::Revisit, Case::OtherKind] {
        let rows = push_trace(case);
        let at = rows.iter().position(|r| r.captured && r.top == "library").expect("the Library was captured at the floor");
        let (page, grid, loading, section, tiles) = rows[at].content.unwrap();
        // Every frame the floor image is what is on screen: up to the settled replacement.
        let shown = 1 + rows[at + 1..].iter().take_while(|r| !r.captured && r.image.is_some()).count();
        let (.., then_section, then_tiles) = rows[at + shown].content.unwrap();
        if loading || tiles == 0 || page * grid < 0.99 || section != 1 {
            bad.push(format!("{case:?}: the floor image shows section {section} with {tiles} tiles (loading={loading}, fade {:.2}) for {shown} frames, then section {then_section} with {then_tiles} tiles in one frame", page * grid));
        }
    }
    assert!(bad.is_empty(), "a warm Library push arrived as an empty image: {bad:#?}");
}

/// The bridge side of a push: a warm store with Movies (section 0) and TV Shows (section 1),
/// pointing at `current`. Returns the key of the section the store points at after
/// `aim_library(kind)`.
fn aimed(current: usize, kind: plx_data::browse::SecKind) -> i64 {
    struct Cleanup;
    impl Drop for Cleanup { fn drop(&mut self) { plx_plex::plex::reset_servers_for_test(); } }
    let _cleanup = Cleanup;
    let session = plx_plex::plex::session::TempSession::new("library-push-aim");
    session.watching("u-library-push-aim");
    plx_plex::plex::reset_servers_for_test();
    let sid = plx_plex::plex::register_for_test("aim-own", "127.0.0.1", 9, "synthetic", "fixture");
    let shared = plx_plex::plex::register_for_test("aim-shared", "127.0.0.1", 10, "synthetic", "fixture");
    plx_plex::plex::set_current(sid);
    let mut rig = Bridge::for_test(|| 0);
    rig.stores.browse.borrow_mut().seed_registered_table_for_test([sid, shared]);
    rig.refresh_browse_directory();
    rig.browse_run(plx_data::stores::browse::BrowseCmd::SetCur(current));
    rig.refresh_browse_directory();
    rig.aim_library(kind);
    rig.refresh_browse_directory();
    let directory = rig.directory.view();
    directory.sections()[directory.current().expect("the store points at a library")].key
}

/// Without a different saved preference, aiming a tab keeps its current library when its kind
/// already matches and switches when it does not.
#[test]
fn aiming_without_a_different_saved_choice_keeps_the_matching_kind() {
    let _guard = plx_base::testlock::serial();
    use plx_data::browse::SecKind::{Movie, Show};
    let movies = aimed(0, Movie);
    let shows = aimed(1, Show);
    assert_ne!(movies, shows, "premise: the two sections are distinct");
    assert_eq!(aimed(1, Movie), movies, "a press on Movies leaves Shows for Movies");
    assert_eq!(aimed(0, Show), shows, "a press on Shows leaves Movies for Shows");
    assert_eq!((aimed(0, Movie), aimed(1, Show)), (movies, shows), "the same kind selects nothing");
}

/// A new Browse owner starts on the first movie library, while the profile's saved choices
/// already name other libraries. This is a restart, not a tab switch during the same run.
fn cold_tab_with_saved_library(kind: plx_data::browse::SecKind) {
    struct Cleanup;
    impl Drop for Cleanup { fn drop(&mut self) { plx_plex::plex::reset_servers_for_test(); } }
    let _cleanup = Cleanup;
    let session = plx_plex::plex::session::TempSession::new("library-cold-tab-choice");
    let user = "u-library-cold-tab-choice";
    session.watching(user);
    plx_plex::plex::reset_servers_for_test();
    let own = plx_plex::plex::register_for_test("cold-tab-own", "127.0.0.1", 9, "synthetic", "fixture");
    let shared = plx_plex::plex::register_for_test("cold-tab-share", "127.0.0.1", 10, "synthetic", "fixture");
    plx_plex::plex::set_current(own);
    assert!(plx_plex::plex::session::update(|current| {
        let mut next = current.clone();
        let mut last = plx_plex::plex::session::LastLibrary { user: user.into(), ..Default::default() };
        last.set("movie", "cold-tab-own", 3);
        last.set("show", "cold-tab-own", 4);
        next.prefs.last_library = vec![last];
        Some(next)
    }));
    let mut rig = Bridge::for_test(|| 0);
    {
        let mut browse = rig.stores.browse.borrow_mut();
        browse.seed_registered_table_for_test([own, shared]);
        browse.append_section_for_test(0, 3, "Other films", plx_data::browse::SecKind::Movie);
        browse.append_section_for_test(0, 4, "Other series", plx_data::browse::SecKind::Show);
    }
    rig.refresh_browse_directory();
    let directory = rig.directory.view();
    assert_eq!(directory.current(), Some(0), "startup still points at the first movie library");
    let wanted = directory.kind_address(kind).expect("a saved favourite of this kind");
    assert_eq!(wanted.section, if kind == plx_data::browse::SecKind::Movie { 3 } else { 4 });
    let mut pages = Dispatcher::<AppHost>::new();
    let tab = if kind == plx_data::browse::SecKind::Movie { HomeTab::Movies } else { HomeTab::Shows };
    nav_tab(&mut pages, &mut rig, tab, None, None);
    rig.refresh_browse_directory();
    let directory = rig.directory.view();
    let current = &directory.sections()[directory.current().unwrap()];
    assert_eq!((current.sid, current.key), (Some(wanted.sid), wanted.section),
        "a cold tab press must restore the saved library even when the initial type matches");

    // Once a tab has been visited, the current section is meaningful. Do not overwrite a
    // same-kind selection with an older persisted preference (a write may still be pending).
    let other = if kind == plx_data::browse::SecKind::Movie { 0 } else { 1 };
    rig.browse_run(plx_data::stores::browse::BrowseCmd::SetCur(other));
    rig.refresh_browse_directory();
    nav_tab(&mut pages, &mut rig, tab, None, None);
    rig.refresh_browse_directory();
    assert_eq!(rig.directory.view().current(), Some(other), "the in-session choice stays selected");

    // An explicitly visited non-favourite must also remain in place during the same run.
    let unpinned = other + 2;
    assert!(!rig.directory.view().sections()[unpinned].row.pinned);
    rig.browse_run(plx_data::stores::browse::BrowseCmd::SetCur(unpinned));
    rig.refresh_browse_directory();
    nav_tab(&mut pages, &mut rig, tab, None, None);
    rig.refresh_browse_directory();
    assert_eq!(rig.directory.view().current(), Some(unpinned), "a same-kind visit is not reset to a favourite");

    // A profile/session reset starts a new first entry, even though the Bridge is retained.
    rig.browse_run(plx_data::stores::browse::BrowseCmd::Reset);
    assert!(rig.mounter.library_kind.is_none());
    assert!(rig.library_commands.is_empty());
    {
        let mut browse = rig.stores.browse.borrow_mut();
        browse.seed_registered_table_for_test([own, shared]);
        browse.append_section_for_test(0, 3, "Other films", plx_data::browse::SecKind::Movie);
        browse.append_section_for_test(0, 4, "Other series", plx_data::browse::SecKind::Show);
    }
    rig.refresh_browse_directory();
    let mut pages = Dispatcher::<AppHost>::new();
    nav_tab(&mut pages, &mut rig, tab, None, None);
    rig.refresh_browse_directory();
    let directory = rig.directory.view();
    let current = &directory.sections()[directory.current().unwrap()];
    assert_eq!((current.sid, current.key), (Some(wanted.sid), wanted.section),
        "reset must not carry the previous session's library intent into the new one");
}

#[test]
fn a_cold_movies_tab_restores_the_saved_library_instead_of_the_first_movie_library() {
    let _guard = plx_base::testlock::serial();
    cold_tab_with_saved_library(plx_data::browse::SecKind::Movie);
}

#[test]
fn a_cold_shows_tab_restores_the_saved_library() {
    let _guard = plx_base::testlock::serial();
    cold_tab_with_saved_library(plx_data::browse::SecKind::Show);
}
