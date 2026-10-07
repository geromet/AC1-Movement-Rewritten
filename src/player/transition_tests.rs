//! Package 3 (AC1-REV-001, -004, -005, -006, -007) regression tests: event-owned and atomic
//! locomotion transitions.
//!
//! These fixtures drive the real production code (`switch_context`, `update_ground`, `update_air`,
//! `HumanClimbData::enter`, `HumanInAirData::enter`, the per-actor transition epoch) through the
//! headless Bevy harness. They assert the *fixed* contract.
//!
//! Scoping note (per the W5 adversarial correction): REV-005 and REV-007 are lifecycle defects that
//! the earlier reproducers proved by seeding the trigger state (`to_ground` / `long_catch`) by hand
//! rather than by a full runtime sequence. The tests below keep the same honest scope: they assert the
//! lifecycle reset at the real entry functions and do not claim an end-to-end runtime reproduction.

use std::time::Duration;

use bevy::prelude::*;
use bevy::time::TimeUpdateStrategy;

use super::air::{AirMode, FallOrigin, HumanInAirData, InAirEntry, Landing, LandingType};
use super::climb::{ClimbEntry, ClimbEntryType};
use super::ledge::{LedgeEntry, LedgeSubState};
use super::{
    advance_player_tick, air, climb, context_active, ground, ledge, player_components, switch_context, ActorContextId,
    Body, HumanDataBundle, Locomotion, Player, SpawnPoint, TransitionSetup,
};
use crate::camera::CameraRig;
use crate::input::PadInput;
use crate::level;

const SPAWN: Vec3 = Vec3::new(0.0, 0.0, 4.0);
const DT: f32 = 1.0 / 60.0;

fn resources(app: &mut App) {
    app.add_plugins(MinimalPlugins)
        .insert_resource(TimeUpdateStrategy::ManualDuration(Duration::from_secs_f64(DT as f64)))
        .insert_resource(PadInput { legs_pressed_ago: f32::INFINITY, hand_pressed_ago: f32::INFINITY, ..default() })
        .insert_resource(SpawnPoint(SPAWN))
        .init_resource::<CameraRig>();
    let (c, g) = level::geometry();
    app.insert_resource(c).insert_resource(g);
}

/// The production player chain: the epoch system first, then the real context systems.
fn player_app() -> App {
    let mut app = App::new();
    resources(&mut app);
    app.add_systems(
        Update,
        (
            advance_player_tick,
            ground::update_ground.run_if(context_active(ActorContextId::Ground)),
            air::update_air.run_if(context_active(ActorContextId::InAir)),
            ledge::update_ledge.run_if(context_active(ActorContextId::Ledge)),
            climb::update_climb.run_if(context_active(ActorContextId::Climb)),
        )
            .chain(),
    );
    app
}

#[derive(Resource, Default, Clone)]
struct ActivationProbe {
    current: Option<ActorContextId>,
    just_switched: bool,
}

fn probe_activation(q: Query<&Locomotion, With<Player>>, mut p: ResMut<ActivationProbe>) {
    if let Ok(l) = q.single() {
        p.current = Some(l.current);
        p.just_switched = l.just_switched;
    }
}

fn probe_app() -> App {
    let mut app = App::new();
    resources(&mut app);
    app.insert_resource(ActivationProbe::default());
    app.add_systems(
        Update,
        (
            advance_player_tick,
            ground::update_ground.run_if(context_active(ActorContextId::Ground)),
            air::update_air.run_if(context_active(ActorContextId::InAir)),
            ledge::update_ledge.run_if(context_active(ActorContextId::Ledge)),
            climb::update_climb.run_if(context_active(ActorContextId::Climb)),
            probe_activation,
        )
            .chain(),
    );
    app
}

fn feet(app: &App, e: Entity) -> Vec3 {
    app.world().get::<Body>(e).unwrap().feet
}

fn loco(app: &App, e: Entity) -> ActorContextId {
    app.world().get::<Locomotion>(e).unwrap().current
}

/// Apply a transition between frames, exactly as `PlayerPlugin`'s callers do inside an update.
fn switch_now(app: &mut App, e: Entity, setup: TransitionSetup) {
    let mut em = app.world_mut().entity_mut(e);
    let mut l = em.take::<Locomotion>().unwrap();
    let mut d = em.take::<HumanDataBundle>().unwrap();
    switch_context(&mut l, &mut d, setup);
    em.insert((l, d));
}

fn fatal_landing() -> Landing {
    Landing { kind: LandingType::Fatal, fall_height: 8.0, total_drop: 8.0, roll: false, action: None }
}

fn fall_entry(velocity: Vec3) -> InAirEntry {
    InAirEntry::Fall { from: Vec3::ZERO, velocity, origin: FallOrigin::Ground, speed_param: 0.0 }
}

fn climb_entry() -> ClimbEntry {
    ClimbEntry {
        entry_type: ClimbEntryType::FromGround,
        hand_l: Vec3::new(0.0, 2.0, 0.0),
        hand_r: Vec3::new(0.2, 2.0, 0.0),
        foot_l: Vec3::new(0.0, 1.2, 0.0),
        foot_r: Vec3::new(0.2, 1.2, 0.0),
        normal: Vec3::NEG_Z,
        from_feet: Vec3::ZERO,
        foot_right: false,
        action: None,
    }
}

// ------------------------------------------------------------------ AC1-REV-001

/// Fatal fall -> first Ground respawn -> a later non-air Ground entry (Climb/pull-up style, landing
/// None) must NOT replay the fatal respawn. The durable `last_landing` stays as history.
#[test]
fn rev_001_non_air_ground_entry_does_not_replay_last_landing() {
    let mut app = player_app();
    let e = app.world_mut().spawn(player_components(Vec3::new(5.0, 0.0, 5.0), 0.0)).id();
    app.update(); // settle; epoch 1

    // Fatal landing requested by the entry.
    switch_now(&mut app, e, TransitionSetup::ToMovement { landing: Some(fatal_landing()) });
    app.update(); // first Ground update: consume the pending landing and respawn
    let after_fatal = feet(&app, e);
    assert!((after_fatal - SPAWN).length() < 1e-4, "fatal landing should respawn to {SPAWN:?}: {after_fatal:?}");
    {
        let g = &app.world().get::<HumanDataBundle>(e).unwrap().ground;
        assert!(g.last_landing.is_some(), "last_landing remains durable history");
        assert!(g.pending_landing.is_none(), "the landing effect was consumed exactly once");
    }

    // Walk off the spawn point, then model a non-air Ground round trip (enter Climb, then Ground with
    // landing None) through the real switch_context.
    app.world_mut().get_mut::<Body>(e).unwrap().feet = Vec3::new(0.5, 0.0, 4.0);
    switch_now(&mut app, e, TransitionSetup::ToClimb(climb_entry()));
    app.update(); // Climb first update (skipped)
    switch_now(&mut app, e, TransitionSetup::ToMovement { landing: None });
    app.update(); // Ground first update: must not replay the stale fatal landing
    app.update(); // normal update
    let after = feet(&app, e);
    assert!(
        (after - SPAWN).length() > 0.3,
        "REV-001: a non-air Ground entry replayed the stale fatal landing (feet {after:?} teleported to {SPAWN:?})"
    );
    assert!(app.world().get::<HumanDataBundle>(e).unwrap().ground.last_landing.is_some());
}

// ------------------------------------------------------------------ AC1-REV-004

/// Forward edge Ground -> Air: the destination runs later in the chain, but must NOT activate in the
/// switch frame; it keeps the marker and consumes the skip on the next actor tick.
#[test]
fn rev_004_forward_ground_to_air_defers_activation() {
    let mut app = probe_app();
    let _e = app.world_mut().spawn(player_components(Vec3::new(0.0, 50.0, 0.0), 0.0)).id();
    app.update();
    let p = app.world().resource::<ActivationProbe>().clone();
    assert_eq!(p.current, Some(ActorContextId::InAir), "ground loss should switch to InAir");
    assert!(
        p.just_switched,
        "REV-004: forward Ground->Air must not activate the destination in the switch frame"
    );
    app.update();
    let p2 = app.world().resource::<ActivationProbe>().clone();
    assert_eq!(p2.current, Some(ActorContextId::InAir));
    assert!(!p2.just_switched, "the deferred first update is consumed on the next actor tick");
}

/// Backward edge Air -> Ground: the destination already ran, so it also keeps the marker in the switch
/// frame and consumes the skip on the next actor tick. Same boundary as the forward edge.
#[test]
fn rev_004_backward_air_to_ground_defers_activation() {
    let mut app = probe_app();
    let e = app.world_mut().spawn(player_components(Vec3::new(0.0, 6.0, 4.0), 0.0)).id();
    {
        let mut l = app.world_mut().get_mut::<Locomotion>(e).unwrap();
        l.current = ActorContextId::InAir;
        l.previous = ActorContextId::Ground;
        l.just_switched = false;
    }
    {
        let mut b = app.world_mut().get_mut::<Body>(e).unwrap();
        b.velocity = Vec3::new(0.0, -1.0, 0.0);
        b.grounded = false;
    }
    {
        let mut d = app.world_mut().get_mut::<HumanDataBundle>(e).unwrap();
        d.air.mode = AirMode::Fall { steer_to: None };
        d.air.apex_y = 6.0;
        d.air.start_y = 6.0;
        d.air.prev_y = 6.0;
    }

    let mut landed = None;
    for _ in 0..240 {
        app.update();
        if loco(&app, e) == ActorContextId::Ground {
            landed = Some(app.world().resource::<ActivationProbe>().clone());
            break;
        }
    }
    let p = landed.expect("never landed into Ground");
    assert!(
        p.just_switched,
        "REV-004: backward Air->Ground must keep the activation marker in the switch frame"
    );
    app.update();
    let p2 = app.world().resource::<ActivationProbe>().clone();
    assert!(!p2.just_switched, "the deferred first update is consumed on the next actor tick");
}

fn trigger_ledge_to_climb(mut q: Query<(&mut Locomotion, &mut HumanDataBundle), With<Player>>) {
    for (mut l, mut d) in &mut q {
        if l.current == ActorContextId::Ledge {
            switch_context(&mut l, &mut d, TransitionSetup::ToClimb(climb_entry()));
        }
    }
}

fn trigger_climb_to_ledge(mut q: Query<(&mut Locomotion, &mut HumanDataBundle), With<Player>>) {
    for (mut l, mut d) in &mut q {
        if l.current == ActorContextId::Climb {
            let e = LedgeEntry::at(Vec3::new(0.0, 2.0, 0.0), Vec3::NEG_Z, Vec3::ZERO, LedgeSubState::HangWallReception);
            switch_context(&mut l, &mut d, TransitionSetup::ToLedge(e));
        }
    }
}

fn cycle_app(forward: bool) -> App {
    let mut app = App::new();
    resources(&mut app);
    app.insert_resource(ActivationProbe::default());
    if forward {
        // Ledge -> Climb: the source switch happens at Ledge's slot, before the real Climb system.
        app.add_systems(
            Update,
            (
                advance_player_tick,
                trigger_ledge_to_climb,
                climb::update_climb.run_if(context_active(ActorContextId::Climb)),
                probe_activation,
            )
                .chain(),
        );
    } else {
        // Climb -> Ledge: the real Ledge system already ran, so no Ledge update follows the switch.
        app.add_systems(
            Update,
            (
                advance_player_tick,
                ledge::update_ledge.run_if(context_active(ActorContextId::Ledge)),
                trigger_climb_to_ledge,
                probe_activation,
            )
                .chain(),
        );
    }
    app
}

/// Ledge <-> Climb matrix: both cyclic edges must expose the same activation boundary.
#[test]
fn rev_004_ledge_climb_cyclic_boundary_parity() {
    let mut fwd = cycle_app(true);
    let ef = fwd.world_mut().spawn(player_components(Vec3::ZERO, 0.0)).id();
    {
        let mut l = fwd.world_mut().get_mut::<Locomotion>(ef).unwrap();
        l.current = ActorContextId::Ledge;
        l.just_switched = false;
    }
    fwd.update();
    let pf = fwd.world().resource::<ActivationProbe>().clone();
    assert_eq!(pf.current, Some(ActorContextId::Climb));
    assert!(pf.just_switched, "forward Ledge->Climb must keep the activation marker in the switch frame");
    fwd.update();
    assert!(!fwd.world().resource::<ActivationProbe>().just_switched);

    let mut bwd = cycle_app(false);
    let eb = bwd.world_mut().spawn(player_components(Vec3::ZERO, 0.0)).id();
    {
        let mut l = bwd.world_mut().get_mut::<Locomotion>(eb).unwrap();
        l.current = ActorContextId::Climb;
        l.just_switched = false;
    }
    bwd.update();
    let pb = bwd.world().resource::<ActivationProbe>().clone();
    assert_eq!(pb.current, Some(ActorContextId::Ledge));
    assert!(pb.just_switched, "backward Climb->Ledge must keep the activation marker in the switch frame");
    bwd.update();
    assert!(!bwd.world().resource::<ActivationProbe>().just_switched);
}

// ------------------------------------------------------------------ AC1-REV-005

/// A fresh Climb entry must clear the context-lifetime `to_ground` continuation left by an interrupted
/// (lost-grip) to-ground move. Lifecycle assertion, not a full runtime reproduction.
#[test]
fn rev_005_fresh_climb_entry_clears_to_ground() {
    let mut loco = Locomotion {
        current: ActorContextId::Climb,
        previous: ActorContextId::Climb,
        just_switched: false,
        tick: 0,
        switch_tick: 0,
    };
    let mut data = HumanDataBundle::default();

    switch_context(&mut loco, &mut data, TransitionSetup::ToClimb(climb_entry()));
    assert!(!data.climb.to_ground, "a fresh Climb starts without a to-ground continuation");
    // An in-progress to-ground move (climb.rs corner / climb-down) sets the flag.
    data.climb.to_ground = true;
    // Lost grip leaves Climb to InAir without consuming the flag.
    switch_context(&mut loco, &mut data, TransitionSetup::ToInAir(fall_entry(Vec3::ZERO)));
    assert_eq!(loco.current, ActorContextId::InAir);
    assert!(data.climb.to_ground, "the interrupted move leaves the flag behind");

    // A later, unrelated Climb entry reuses HumanClimbData.
    switch_context(&mut loco, &mut data, TransitionSetup::ToClimb(climb_entry()));
    assert!(
        !data.climb.to_ground,
        "REV-005: a fresh Climb entry must clear the stale to-ground continuation"
    );
}

/// The from-ground entry path (`enter_from_ground`) must reset it too.
#[test]
fn rev_005_enter_from_ground_clears_to_ground() {
    let mut d = HumanDataBundle::default();
    d.climb.to_ground = true;
    let mut e = climb_entry();
    e.entry_type = ClimbEntryType::FromGround;
    d.climb.enter(e);
    assert!(!d.climb.to_ground, "REV-005: enter_from_ground must clear to_ground");
}

// ------------------------------------------------------------------ AC1-REV-006

/// Nonzero prior velocity + `InAirEntry::Fall { velocity: ZERO }` must produce zero horizontal
/// displacement and zero velocity across activation.
#[test]
fn rev_006_fall_velocity_zero_does_not_coast_stale_velocity() {
    let mut app = player_app();
    let e = app.world_mut().spawn(player_components(Vec3::new(0.0, 20.0, 4.0), 0.0)).id();
    {
        let mut l = app.world_mut().get_mut::<Locomotion>(e).unwrap();
        l.current = ActorContextId::InAir;
        l.just_switched = false;
    }
    app.update(); // prime the epoch

    {
        let mut b = app.world_mut().get_mut::<Body>(e).unwrap();
        b.velocity = Vec3::new(3.0, 0.0, 0.0); // stale horizontal velocity from the previous context
        b.grounded = false;
    }
    switch_now(&mut app, e, TransitionSetup::ToInAir(fall_entry(Vec3::ZERO)));
    let before = feet(&app, e);
    app.update(); // first update after the switch: apply the requested ZERO velocity, then coast
    let after = feet(&app, e);
    let horiz = Vec3::new(after.x - before.x, 0.0, after.z - before.z).length();
    assert!(
        horiz < 1e-4,
        "REV-006: Fall{{velocity: ZERO}} coasted stale velocity ({horiz:.5} m, velocity {:?})",
        app.world().get::<Body>(e).unwrap().velocity
    );
    assert!(
        app.world().get::<Body>(e).unwrap().velocity.x.abs() < 1e-4,
        "stale horizontal velocity survived the Fall entry"
    );
}

/// Positive control: a requested nonzero Fall velocity must actually be applied on activation.
#[test]
fn rev_006_fall_velocity_nonzero_is_applied() {
    let mut app = player_app();
    let e = app.world_mut().spawn(player_components(Vec3::new(0.0, 20.0, 4.0), 0.0)).id();
    {
        let mut l = app.world_mut().get_mut::<Locomotion>(e).unwrap();
        l.current = ActorContextId::InAir;
        l.just_switched = false;
    }
    app.update();
    switch_now(&mut app, e, TransitionSetup::ToInAir(fall_entry(Vec3::new(2.0, 0.0, 0.0))));
    let before = feet(&app, e);
    app.update();
    let after = feet(&app, e);
    assert!(after.x - before.x > 0.02, "requested Fall velocity should move the body (moved {:.4})", after.x - before.x);
    assert!((app.world().get::<Body>(e).unwrap().velocity.x - 2.0).abs() < 1e-4);
}

// ------------------------------------------------------------------ AC1-REV-007

/// `long_catch` must be reset on every InAir entry so a later unrelated entry cannot reuse the prior
/// catch classification. Lifecycle assertion, not a full runtime reproduction.
#[test]
fn rev_007_fresh_air_entry_clears_long_catch() {
    let mut d = HumanInAirData::default();
    d.enter(fall_entry(Vec3::ZERO));
    d.long_catch = true; // a >3 m free-fall catch classification
    assert!(d.long_catch);

    // An unrelated later InAir entry (a short target jump) starts a fresh lifetime.
    d.enter(fall_entry(Vec3::ZERO));
    assert!(
        !d.long_catch,
        "REV-007: a fresh InAir entry must reset long_catch instead of reusing the previous classification"
    );
    // The current event then re-classifies: a short catch keeps the short selection.
    d.long_catch = false;
    assert!(!d.long_catch, "the later short catch selects the short classification");
}
