//! Final BACK confirmation, composed from the shared decision prompt on a modal surface.
use crate::registry::{AppFx, AppLike, LoopReq};
use plx_machine::machine::{Canon, Cx, Effects, EntryId, FocusKey, Fx, GroupId, Handled, LogicalState, Machine, NavOp};
use plx_ui::decision_prompt::{DecisionPrompt, PromptStep};
use plx_ui::screen::{At, Dir, DrawFrame, Focusable, GroupSpec, Placed, RenderStrategy, Screen, ScreenEvent, Step};

pub const SHAPE: &str = "ExitConfirmation{answered:bool,open:bool,affirm:bool,scroll_target_bits:u32}";

pub struct ExitConfirmation {
    entry: EntryId,
    prompt: DecisionPrompt,
    answered: bool,
}
impl ExitConfirmation {
    pub fn new(entry: EntryId) -> Self {
        let mut prompt = DecisionPrompt::new(GroupId(0), 0, 1,
            plx_platform::i18n::msg::settings_cancel_c(), plx_platform::i18n::msg::browse_exit_confirm_c());
        prompt.open(plx_platform::i18n::msg::browse_exit_question_c(), plx_platform::i18n::msg::browse_exit_body());
        Self { entry, prompt, answered: false }
    }
}
impl<H: AppLike> Machine<H> for ExitConfirmation {
    type Ev = ScreenEvent<H>;
    fn step(&mut self, ev: &Self::Ev, cx: &Cx<'_, H>, fx: &mut Effects<'_, H>) -> Handled {
        if let ScreenEvent::Tick(t) = ev { self.prompt.update(t.dt()); }
        match self.prompt.step(ev, cx) {
            PromptStep::Answer(quit) => {
                if !self.answered {
                    self.answered = true;
                    if quit { fx.push(Fx::App(AppFx::Loop(LoopReq::QuitConfirmed))); }
                    fx.push(Fx::Nav(NavOp::Dismiss(self.entry)));
                }
                Handled::Yes
            }
            PromptStep::Done(handled) => handled,
            PromptStep::Pass => Handled::No,
        }
    }
}
impl<H: AppLike> Focusable<H> for ExitConfirmation {
    fn groups(&self, cx: &Cx<'_, H>, out: &mut Vec<GroupSpec>) { self.prompt.groups(out, cx.measure); }
    fn group_of(&self, key: &u32, _: &Cx<'_, H>) -> Option<GroupId> { self.prompt.group_of(*key).flatten() }
    fn neighbour(&self, key: FocusKey<u32>, dir: Dir, _: &Cx<'_, H>) -> Step<u32> {
        self.prompt.neighbour(key, dir).unwrap_or(Step::Edge)
    }
    fn place(&self, key: &u32, cx: &Cx<'_, H>, _: At) -> Option<Placed> { self.prompt.place(*key, cx.measure).flatten() }
    fn reconcile(&self, key: FocusKey<u32>, _: &Cx<'_, H>) -> FocusKey<u32> {
        self.prompt.reconcile(key).unwrap_or(key)
    }
    fn seat(&self, group: GroupId, _: Placed, _: &Cx<'_, H>) -> FocusKey<u32> {
        self.prompt.seat(group, self.entry).unwrap_or(FocusKey { entry: self.entry, elem: 0 })
    }
}
impl<H: AppLike> Screen<H> for ExitConfirmation {
    fn as_any(&self) -> Option<&dyn std::any::Any> { Some(self) }
    fn crumb(&self, _: &Cx<'_, H>) -> Option<std::borrow::Cow<'_, str>> { None }
    fn name(&self) -> &'static str { "exitconfirm" }
    fn state(&self) -> &dyn LogicalState { self }
    fn prepare(&mut self, _: &mut plx_ui::frame::Budget, _: &Cx<'_, H>) {}
    fn draw(&mut self, f: &mut DrawFrame<'_, '_, H>) { self.prompt.draw(f, self.entry); }
    fn render(&self) -> RenderStrategy { RenderStrategy::Page }
}
impl LogicalState for ExitConfirmation {
    fn write(&self, c: &mut Canon) {
        c.bool(self.answered).bool(self.prompt.is_open()).bool(self.prompt.choice()).u32(self.prompt.scroll_target_bits());
    }
    fn probe(&self, out: &mut String) { out.push_str("exitconfirm"); }
}
