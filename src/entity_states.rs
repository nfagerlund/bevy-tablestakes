use crate::{
    assets_setup::*, behaviors::*, char_animation::*, compass::flip_angle, debug_settings::*,
    input::CurrentInputs, movement::*, phys_space::PhysTransform,
};
use bevy::ecs::system::EntityCommands;
use bevy::prelude::*;
use bevy::utils::Duration;
use bevy_prng::Xoshiro256Plus;
use bevy_rand::prelude::*;
use rand::prelude::Rng;

// ------- Types -------

pub type EnemyStateMachine = EntityStateMachine<EnemyState>;
pub type PlayerStateMachine = EntityStateMachine<PlayerState>;
type GameRNG = GlobalEntropy<Xoshiro256Plus>;

#[derive(Component, Reflect, Default)]
pub struct StateTimer(pub Option<Timer>);

#[derive(Component)]
pub struct EntityStateMachine<T>
where
    T: Clone,
{
    // fields are private
    current: T,
    next: Option<T>,
}

impl<T: Clone> EntityStateMachine<T> {
    pub fn new(current: T) -> Self {
        Self {
            current: current.clone(),
            // Make sure we run sprite/behavior/timer setup on first tick!
            next: Some(current),
        }
    }
    pub fn push_transition(&mut self, next: T) {
        self.next = Some(next);
    }
    // fn has_transition(&self) -> bool {
    //     self.next.is_some()
    // }
    // fn peek_next(&self) -> Option<&PlayerState> {
    //     self.next.as_ref()
    // }
    pub fn current(&self) -> &T {
        &self.current
    }
    pub fn _current_mut(&mut self) -> &mut T {
        &mut self.current
    }
    /// If a transition is queued up, switch to the next state, then call the provided
    /// closure, passing it a mutable reference to self. The closure will see the new
    /// state when it checks current / current_mut. The closure only gets called
    /// if there's a transition waiting to go.
    pub fn do_transition(&mut self, f: impl FnOnce(&mut Self)) {
        if let Some(next) = self.next.take() {
            self.current = next;
            f(self);
        }
    }
}

#[derive(Clone)]
pub enum PlayerState {
    Idle,
    Run,
    Roll { roll_input: Vec2 },
    Bonk { bonk_input: Vec2, distance: f32 },
    Attack,
}

impl PlayerState {
    pub const ROLL_DISTANCE: f32 = 52.0;
    pub const BONK_FROM_ROLL_DISTANCE: f32 = 18.0;
    pub const BONK_Z_VELOCITY: f32 = 65.0;
    pub const ROLL_SPEED: f32 = Speed::ROLL;
    pub const ATTACK_DURATION_MS: u64 = 400;

    pub fn timer(&self) -> Option<Timer> {
        match self {
            PlayerState::Idle => None,
            PlayerState::Run => None,
            PlayerState::Roll { .. } => {
                let duration_secs = Self::ROLL_DISTANCE / Self::ROLL_SPEED;
                Some(Timer::from_seconds(duration_secs, TimerMode::Once))
            },
            PlayerState::Bonk { .. } => None,
            PlayerState::Attack => Some(Timer::new(
                Duration::from_millis(Self::ATTACK_DURATION_MS),
                TimerMode::Once,
            )),
        }
    }

    pub fn animation_data(&self) -> (Ases, Playback, Option<u64>) {
        match self {
            PlayerState::Idle => (Ases::TkIdle, Playback::Loop, None),
            PlayerState::Run => (Ases::TkRun, Playback::Loop, None),
            PlayerState::Roll { .. } => {
                let duration = (Self::ROLL_DISTANCE / Self::ROLL_SPEED * 1000.0) as u64;
                (Ases::TkRoll, Playback::Once, Some(duration))
            },
            PlayerState::Bonk { .. } => (Ases::TkHurt, Playback::Once, None), // one frame, so no duration :)
            PlayerState::Attack => (
                Ases::TkSlash,
                Playback::Once,
                Some(Self::ATTACK_DURATION_MS),
            ),
        }
    }

    /// Given an EntityCommands instance, add and remove the appropriate
    /// behavioral components on that entity. TBH I'd rather "just" return
    /// a set of behaviors, but actually that's fiendishly complicated
    /// because those types are all different, so we do it the easy way.
    pub fn set_behaviors(&self, mut cmds: EntityCommands, numbers: &NumbersSettings) {
        cmds.remove::<AllBehaviors>();
        match self {
            PlayerState::Idle => {
                cmds.insert(MobileFree);
            },
            PlayerState::Run => {
                cmds.insert(MobileFree);
            },
            PlayerState::Roll { roll_input } => {
                cmds.insert((
                    MobileFixed {
                        input: *roll_input,
                        face: true,
                    },
                    Headlong,
                ));
            },
            PlayerState::Bonk { bonk_input, .. } => {
                cmds.insert((
                    MobileFixed {
                        input: *bonk_input,
                        face: false,
                    },
                    Hitstun,
                    Knockback,
                    Launch {
                        z_velocity: numbers.player_bonk_z_velocity,
                    },
                ));
            },
            PlayerState::Attack => {
                cmds.insert((MobileFixed {
                    input: Vec2::ZERO,
                    face: false,
                },));
            },
        }
    }

    // TODO: I'm scaling this one for now anyway, but, it'd be good to learn the length of a state
    // based on its sprite asset, so it can be *dictated* by the source file but not *managed*
    // by the animation system. ...Cache it with a startup system?
    pub fn attack() -> Self {
        Self::Attack
    }

    pub fn roll(direction: f32) -> Self {
        Self::Roll {
            roll_input: Vec2::from_angle(direction),
        }
    }

    pub fn bonk_from_vector(v: Vec2) -> Self {
        Self::Bonk {
            bonk_input: v.normalize_or_zero(),
            distance: v.length(),
        }
    }
}

#[derive(Clone)]
pub enum EnemyState {
    Idle,
    Patrol { displacement: Vec2 },
    Chase { target: Entity },
    Attack,
    Hurt,
    Dying,
}

impl EnemyState {
    pub fn animation_data(&self) -> (Ases, Playback) {
        match self {
            EnemyState::Idle { .. } => (Ases::SlimeIdle, Playback::Loop),
            EnemyState::Patrol { .. } => (Ases::SlimeIdle, Playback::Loop),
            EnemyState::Chase { .. } => (Ases::SlimeIdle, Playback::Loop),
            EnemyState::Attack => (Ases::SlimeAttack, Playback::Loop),
            EnemyState::Hurt => (Ases::SlimeHurt, Playback::Once),
            EnemyState::Dying => (Ases::SlimeDie, Playback::Once),
        }
    }

    pub fn timer(&self) -> Option<Timer> {
        match self {
            EnemyState::Idle => Some(Timer::from_seconds(2.0, TimerMode::Once)),
            EnemyState::Patrol { displacement, .. } => {
                let duration_secs = displacement.length() / Speed::ENEMY_RUN;
                Some(Timer::from_seconds(duration_secs, TimerMode::Once))
            },
            // TBH I don't think this is correct, but it'll get things moving until I sort out
            // how to wire a limit though to set_behaviors():
            EnemyState::Chase { .. } => Some(Timer::from_seconds(10.0, TimerMode::Once)),
            EnemyState::Attack => todo!(),
            EnemyState::Hurt => todo!(),
            EnemyState::Dying => todo!(),
        }
    }

    const SLIME_AGGRO_RANGE: f32 = 50.0;

    pub fn set_behaviors(&self, mut cmds: EntityCommands) {
        cmds.remove::<AllBehaviors>();
        match self {
            EnemyState::Idle => {
                cmds.insert(AggroRange(Self::SLIME_AGGRO_RANGE));
            },
            EnemyState::Patrol { displacement, .. } => {
                cmds.insert((
                    MobileFixed {
                        input: displacement.normalize_or_zero(),
                        face: true,
                    },
                    AggroRange(Self::SLIME_AGGRO_RANGE),
                ));
            },
            EnemyState::Chase { target } => {
                cmds.insert(Aggro {
                    target: *target,
                    limit: None,
                });
            },
            EnemyState::Attack => todo!(),
            EnemyState::Hurt => todo!(),
            EnemyState::Dying => todo!(),
        }
    }
}

impl Default for EnemyState {
    fn default() -> Self {
        Self::Idle
    }
}

#[derive(Component)]
pub enum PatrolArea {
    Patch { home: Vec2, radius: f32 },
    _Shush, // leave me alone about my irrefutable if lets, man
}

impl PatrolArea {
    pub fn random_destination(&self, rng: &mut impl Rng) -> Vec2 {
        match self {
            PatrolArea::Patch { home, radius } => {
                let angle: f32 = rng.gen_range(-(std::f32::consts::PI)..=std::f32::consts::PI);
                let distance: f32 = rng.gen_range(0.0..*radius);
                *home + Vec2::from_angle(angle) * distance
            },
            PatrolArea::_Shush => todo!(),
        }
    }
}

// ------- Systems -------

/// Hey, how much CAN I get away with processing at this point? I know I want to handle
/// walk/idle transitions here, but..... action button?
pub fn player_state_read_inputs(
    inputs: Res<CurrentInputs>,
    mut player_q: Query<(&mut PlayerStateMachine, &mut Motion)>,
) {
    for (mut machine, mut motion) in player_q.iter_mut() {
        // Moves -- ignored unless run or idle
        let move_input = inputs.movement;
        match machine.current() {
            PlayerState::Idle => {
                if move_input.length() > 0.0 {
                    machine.push_transition(PlayerState::Run);
                }
                motion.face(move_input); // sprite-relevant.
            },
            PlayerState::Run => {
                if move_input.length() == 0.0 {
                    machine.push_transition(PlayerState::Idle);
                }
                motion.face(move_input);
            },
            _ => (),
        }

        // Action button
        if inputs.actioning {
            // Right now there is only roll.
            match machine.current() {
                PlayerState::Idle | PlayerState::Run => {
                    machine.push_transition(PlayerState::roll(motion.facing));
                },
                _ => (),
            }
        }

        // Attack button
        if inputs.attacking {
            match machine.current() {
                PlayerState::Idle | PlayerState::Run => {
                    machine.push_transition(PlayerState::attack());
                },
                _ => (),
            }
        }
    }
}

pub fn player_state_read_events(
    mut rebound_events: EventReader<Rebound>,
    mut landing_events: EventReader<Landed>,
    mut player_q: Query<&mut PlayerStateMachine>,
) {
    for rb in rebound_events.read() {
        if let Ok(mut machine) = player_q.get_mut(rb.entity) {
            machine.push_transition(PlayerState::bonk_from_vector(rb.vector));
        }
    }
    for ld in landing_events.read() {
        if let Ok(mut machine) = player_q.get_mut(ld.0) {
            if let PlayerState::Bonk { .. } = machine.current() {
                machine.push_transition(PlayerState::Idle);
            }
        }
    }
}

/// Near the start of every frame, check whether the player state machine is switching
/// states; if so, handle any setup and housekeeping to make the new state usable on the
/// current frame.
pub fn player_state_changes(
    mut player_q: Query<(
        Entity,
        &mut PlayerStateMachine,
        &mut StateTimer,
        &mut Speed,
        &mut CharAnimationState,
    )>,
    animations_map: Res<AnimationsMap>,
    time: Res<Time>,
    numbers: Res<NumbersSettings>,
    mut commands: Commands,
) {
    for (entity, mut machine, mut state_timer, mut speed, mut animation_state) in
        player_q.iter_mut()
    {
        // FIRST: if a state used up its time allotment last frame (without being interrupted),
        // this is where we queue up a transition to the next state.
        if let Some(ref timer) = state_timer.0 {
            if machine.next.is_none() && timer.finished() {
                match machine.current() {
                    PlayerState::Idle => (), // not timed
                    PlayerState::Run => (),  // not timed
                    PlayerState::Roll { .. } => machine.push_transition(PlayerState::Idle),
                    PlayerState::Bonk { .. } => machine.push_transition(PlayerState::Idle),
                    PlayerState::Attack => machine.push_transition(PlayerState::Idle),
                }
            }
        }

        // SEVERAL-TH: maybe change states, and do setup housekeeping for the new state.
        machine.do_transition(|machine| {
            // SECOND: Set new Option<Timer>
            state_timer.0 = machine.current().timer();

            // THIRD: Update sprite
            let (name, play, time) = machine.current().animation_data();
            if let Some(ani) = animations_map.get(&name) {
                animation_state.change_animation(ani.clone(), play);
                if let Some(run_ms) = time {
                    animation_state.set_total_run_time_to(run_ms);
                }
            } else {
                warn!("Tried to set missing animation {:?} on player", name);
            }

            // FOURTH: Update speed
            speed.0 = match machine.current() {
                PlayerState::Idle => 0.0,
                PlayerState::Run => Speed::RUN,
                PlayerState::Roll { .. } => Speed::ROLL,
                PlayerState::Bonk { .. } => Speed::BONK,
                PlayerState::Attack { .. } => 0.0,
            };

            // FIFTH: Add and remove behavioral components
            machine
                .current()
                .set_behaviors(commands.entity(entity), &numbers);
        });

        // SIXTH: If the current state has a timer, tick it forward.
        if let Some(ref mut timer) = state_timer.0 {
            timer.tick(time.delta());
        }
    }
}

pub fn enemy_state_read_events(
    mut aggroing: EventReader<AggroActivate>,
    mut query: Query<&mut EnemyStateMachine>,
) {
    for aggro in aggroing.read() {
        if let Ok(mut machine) = query.get_mut(aggro.subject) {
            machine.push_transition(EnemyState::Chase {
                target: aggro.target,
            });
        }
    }
}

pub fn enemy_state_changes(
    mut query: Query<(
        Entity,
        &mut EnemyStateMachine,
        &mut StateTimer,
        &mut CharAnimationState,
        &PatrolArea,
        &PhysTransform,
    )>,
    time: Res<Time>,
    mut rng: ResMut<GameRNG>,
    animations_map: Res<AnimationsMap>,
    mut commands: Commands,
) {
    // Going in serial, because I'm using a global RNG still (instead of forking it to each enemy)
    for (entity, mut machine, mut state_timer, mut anim, patrol, transform) in query.iter_mut() {
        // ZEROTH: if a state spent its timer, queue a transition.
        if let Some(ref timer) = state_timer.0 {
            if machine.next.is_none() && timer.finished() {
                match machine.current() {
                    EnemyState::Idle => {
                        // Decide where we're patrolling to next
                        let dest = patrol.random_destination(&mut *rng);
                        let displacement = dest - transform.translation.truncate();
                        machine.push_transition(EnemyState::Patrol { displacement });
                    },
                    EnemyState::Patrol { .. } => {
                        machine.push_transition(EnemyState::Idle);
                    },
                    EnemyState::Chase { .. } => {
                        machine.push_transition(EnemyState::Idle);
                    },
                    EnemyState::Attack => todo!(),
                    EnemyState::Hurt => todo!(),
                    EnemyState::Dying => todo!(),
                }
            }
        }

        // FIRST and SECOND: maybe change states, and do all our setup housekeeping for the new state.
        machine.do_transition(|machine| {
            let current = machine.current();

            // Set new Option<Timer>
            state_timer.0 = current.timer();

            // Update sprite
            let (name, play) = current.animation_data();
            if let Some(ani) = animations_map.get(&name) {
                anim.change_animation(ani.clone(), play);
            } else {
                warn!(
                    "Whoa oops, tried to set animation {:?} on enemy and it whiffed",
                    name
                );
            }

            // THIRD??: add and remove behaviors
            current.set_behaviors(commands.entity(entity));
        });

        // Finally: if the current state has a timer, tick it.
        if let Some(ref mut timer) = state_timer.0 {
            timer.tick(time.delta());
        }
    }
}

/// If player bonked into a wall, queue a state transition.
/// TODO: Generalize knockback. why should this be player-specific? Or bonk-specific?
pub fn player_queue_wall_bonk(
    player_q: Query<(Entity, &Motion), With<Headlong>>,
    mut rebound_events: EventWriter<Rebound>,
) {
    for (entity, motion) in player_q.iter() {
        if let Some(MotionResult { collided: true, .. }) = motion.result {
            // We hit a wall, so bounce back:
            let opposite_direction = flip_angle(motion.facing);
            let distance = PlayerState::BONK_FROM_ROLL_DISTANCE;
            rebound_events.send(Rebound {
                entity,
                vector: Vec2::from_angle(opposite_direction) * distance,
            });
        }
    }
}
