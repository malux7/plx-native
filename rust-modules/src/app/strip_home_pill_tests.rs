//! OK on Home enters the hero; BACK reaches its navigation pill before platform Home.

use super::*;

/// What the Home pill queued for Home's mount, in order.
fn queued(rig: &mut Bridge) -> Vec<plx_screens::registry::HomeCmd> {
    let mut out = Vec::new();
    while let Some(command) = rig.home_commands.pop_front() { out.push(command); }
    out
}

#[test]
fn the_home_pill_seats_the_hero_instead_of_keeping_focus_on_the_strip() {
    use plx_screens::registry::HomeCmd;
    let _guard = plx_base::testlock::serial();
    let mut d = Dispatcher::<AppHost>::new();
    let mut rig = Bridge::for_test(|| 0);
    nav_home_pill(&mut d, &mut rig, None);
    assert_eq!(queued(&mut rig), vec![HomeCmd::Hero],
        "a Home pill press must leave focus in the content, not on the pill it was pressed from");
}

use super::test_support::World;

fn assert_home_focus(w: &World) {
    assert!(w.on(&AppArg::Home));
    assert_eq!(w.focus().map(|key| key.elem), Some(plx_screens::home::STRIP_HOME_ELEM));
}

#[test]
fn back_walks_from_home_shelves_through_hero_and_home_pill_before_platform_home() {
    let mut w = World::home_grid("back-home-ladder", 2, 3);
    w.press(Key::Down);
    assert!(w.focus().is_some_and(|key| key.elem < plx_ui::dispatch::STRIP_BASE && key.elem != 0));
    w.press(Key::Back);
    assert_eq!(w.focus().map(|key| key.elem), Some(0));
    assert!(w.bridge.take_reqs().is_empty());
    w.press(Key::Back);
    assert_home_focus(&w);
    assert!(w.bridge.take_reqs().is_empty(), "the Home pill must precede platform Home");
    w.press(Key::Back);
    assert!(matches!(w.bridge.take_reqs().as_slice(), [LoopReq::BackAtRoot]));
}

#[test]
fn back_from_every_other_home_bar_member_including_settings_lands_on_home() {
    let mut w = World::movies_library("back-every-bar-member", false);
    nav_home_pill(&mut w.d, &mut w.bridge, None);
    w.frames(60);
    let members: Vec<_> = w.d.nav.tabs.strip.iter().map(|member| member.elem)
        .filter(|elem| *elem != plx_screens::home::STRIP_HOME_ELEM).collect();
    assert!(members.contains(&plx_screens::home::STRIP_ACCOUNT_ELEM), "include the settings/account opener");
    for elem in members {
        let page = w.d.nav.top_page().unwrap();
        let entry = page.id;
        let instance = page.inst.as_ref().unwrap().id;
        w.d.emit(MachineId::Nav, Fx::Deliver(MachineId::Instance(instance), Delivery::Screen(
            ScreenEvent::Enter(Enter::Fresh { focus: FocusTarget::Elem(FocusKey { entry, elem }) }))));
        w.frames(2);
        assert_eq!(w.focus().map(|key| key.elem), Some(elem));
        w.press(Key::Back);
        assert_home_focus(&w);
        assert!(w.bridge.take_reqs().is_empty(), "Back on bar member {elem} must not exit");
    }
}

fn library_back_path(tab: HomeTab, own_pill: u32, name: &str) {
    let mut w = World::movies_library(name, true);
    if tab == HomeTab::Shows {
        nav_tab(&mut w.d, &mut w.bridge, tab, None, None);
        w.frames(60);
        w.bridge.stores.browse.borrow_mut().seed_episodes_for_test(60);
        w.frames(60);
    }
    assert_eq!(w.grid_cell(), Some((0, 0)), "the library opens on its first card");
    w.press(Key::Back);
    assert!(w.on(&AppArg::Library));
    assert!(w.grid_cell().is_none() && w.elem().is_some_and(|elem| elem < plx_ui::dispatch::STRIP_BASE),
        "the first BACK reaches the library header");
    w.press(Key::Back);
    assert!(w.on(&AppArg::Library));
    assert_eq!(w.elem(), Some(own_pill), "the second BACK reaches the library's own tab");
    w.press(Key::Back);
    assert_home_focus(&w);
    assert!(w.bridge.take_reqs().is_empty(), "show Home before requesting platform Home");
    w.press(Key::Back);
    assert!(matches!(w.bridge.take_reqs().as_slice(), [LoopReq::BackAtRoot]));
}

#[test]
fn movies_back_shows_home_with_home_focused_before_platform_home() {
    library_back_path(HomeTab::Movies, plx_screens::home::STRIP_MOVIES_ELEM, "back-movies-home-page");
}

#[test]
fn shows_back_shows_home_with_home_focused_before_platform_home() {
    library_back_path(HomeTab::Shows, plx_screens::home::STRIP_SHOWS_ELEM, "back-shows-home-page");
}

#[test]
fn back_on_the_home_pill_of_library_and_search_already_allows_platform_home() {
    let mut w = World::movies_library("back-peer-home-pill", false);
    for tab in [HomeTab::Movies, HomeTab::Search] {
        nav_tab(&mut w.d, &mut w.bridge, tab, None, None);
        w.frames(60);
        let page = w.d.nav.top_page().unwrap();
        let entry = page.id;
        let instance = page.inst.as_ref().unwrap().id;
        w.d.emit(MachineId::Nav, Fx::Deliver(MachineId::Instance(instance), Delivery::Screen(
            ScreenEvent::Enter(Enter::Fresh { focus: FocusTarget::Elem(FocusKey {
                entry, elem: plx_screens::home::STRIP_HOME_ELEM,
            }) }))));
        w.frames(2);
        assert_eq!(w.elem(), Some(plx_screens::home::STRIP_HOME_ELEM), "premise: Home is focused for {tab:?}");
        w.press(Key::Back);
        assert_eq!(w.d.nav.top_page().map(|page| page.id), Some(entry), "BACK must not activate Home for {tab:?}");
        assert_eq!(w.elem(), Some(plx_screens::home::STRIP_HOME_ELEM));
        assert!(matches!(w.bridge.take_reqs().as_slice(), [LoopReq::BackAtRoot]));
    }
}

#[test]
fn back_from_another_hero_action_or_its_shelf_returns_to_play_first() {
    let mut w = World::home_with_disc("back-hero-action", [8; 12]);
    let play = w.elem();
    w.press(Key::Right);
    assert_ne!(w.elem(), play);
    w.press(Key::Back);
    assert_eq!(w.elem(), play);
    w.press(Key::Right);
    w.press(Key::Down);
    w.press(Key::Back);
    assert_eq!(w.elem(), play, "the fold must not restore the last visited secondary hero control");
    assert!(w.bridge.take_reqs().is_empty());
}

#[test]
fn the_last_home_back_opens_a_confirmation_instead_of_backgrounding() {
    let mut w = World::home_grid("confirm-exit-root", 2, 3);
    let before = plx_platform::tv::home::home_requests();
    w.press(Key::Back);
    w.press(Key::Back);
    for req in w.bridge.take_reqs() {
        assert!(crate::app::run::reduce_navigation_request(req, &mut w.d).is_ok());
    }
    assert!(crate::app::run::reduce_navigation_request(LoopReq::BackAtRoot, &mut w.d).is_ok());
    w.frames(60);
    assert!(w.d.nav.modals.top().is_some(), "the final BACK must ask before leaving");
    assert_eq!(w.d.nav.modals.surfaces.len(), 1, "repeated requests must not stack questions");
    assert!(crate::app::run::reduce_navigation_request(LoopReq::BackAtRoot, &mut w.d).is_ok());
    assert_eq!(plx_platform::tv::home::home_requests(), before);
}

fn open_exit_confirmation(w: &mut World) {
    w.press(Key::Back);
    w.press(Key::Back);
    for req in w.bridge.take_reqs() {
        assert!(crate::app::run::reduce_navigation_request(req, &mut w.d).is_ok());
    }
    w.frames(60);
    assert!(matches!(w.d.nav.modals.top().map(|s| &s.entry.arg), Some(AppArg::ExitConfirmation)));
    assert_eq!(w.elem(), Some(0), "Cancel must be the opening choice");
}

#[test]
fn exit_confirmation_cancel_and_back_restore_the_home_focus() {
    let mut w = World::home_grid("confirm-exit-cancel", 2, 3);
    let before = plx_platform::tv::home::home_requests();
    open_exit_confirmation(&mut w);
    w.press(Key::Ok);
    w.frames(60);
    assert!(w.d.nav.modals.top().is_none());
    assert_home_focus(&w);
    assert!(w.bridge.take_reqs().is_empty());
    // Reopen from the Home button, then cancel even with Quit highlighted.
    w.press(Key::Back);
    for req in w.bridge.take_reqs() { assert!(crate::app::run::reduce_navigation_request(req, &mut w.d).is_ok()); }
    w.frames(60);
    w.press(Key::Right);
    assert_eq!(w.elem(), Some(1));
    w.press(Key::Back);
    w.frames(60);
    assert!(w.d.nav.modals.top().is_none());
    assert_home_focus(&w);
    assert!(w.bridge.take_reqs().is_empty());
    assert_eq!(plx_platform::tv::home::home_requests(), before);
}

#[test]
fn only_confirming_quit_requests_home_and_stops_the_application_loop() {
    let mut w = World::home_grid("confirm-exit-quit", 2, 3);
    let before = plx_platform::tv::home::home_requests();
    open_exit_confirmation(&mut w);
    w.press(Key::Right);
    w.press(Key::Ok);
    let requests = w.bridge.take_reqs();
    assert!(matches!(requests.as_slice(), [LoopReq::QuitConfirmed]));
    assert_eq!(plx_platform::tv::home::home_requests(), before, "the dialog only emits an intent");
    let mut running = true;
    crate::app::run::quit_confirmed(&mut running);
    assert!(!running, "the production performer must leave the loop and reach normal process shutdown");
    assert_eq!(plx_platform::tv::home::home_requests(), before + 1);
}
