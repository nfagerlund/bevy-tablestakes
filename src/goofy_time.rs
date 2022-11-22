use std::collections::VecDeque;
use bevy::prelude::*;
use bevy::{
    utils::Duration,
};

pub struct StaticTimePlugin;
impl Plugin for StaticTimePlugin {
    fn build(&self, app: &mut App) {
        app
            .insert_resource(RecentFrameTimes{ buffer: VecDeque::new() })
            .insert_resource(SmoothedTime {
                delta: Duration::new(0, 0),
            })
            .add_system_to_stage(CoreStage::PreUpdate, time_smoothing_system);
    }
}

pub struct SmoothedTimePlugin;
impl Plugin for SmoothedTimePlugin {
    fn build(&self, app: &mut App) {
        app
            .insert_resource(StaticTime);
    }
}

struct RecentFrameTimes {
    buffer: VecDeque<Duration>,
}
struct SmoothedTime {
    delta: Duration,
}

impl SmoothedTime {
    fn delta_seconds(&self) -> f32 {
        self.delta.as_secs_f32()
    }
    fn delta(&self) -> Duration {
        self.delta
    }
}

struct StaticTime;

impl StaticTime {
    fn delta_seconds(&self) -> f32 {
        1./60.
    }
    fn delta(&self) -> Duration {
        Duration::new(1, 0) / 60
    }
}


/// Smooth out delta time before doing anything with it. This is unoptimized, but that might not matter.
fn time_smoothing_system(
    time: Res<Time>,
    mut recent_time: ResMut<RecentFrameTimes>,
    mut smoothed_time: ResMut<SmoothedTime>,
) {
    let window: usize = 11;
    let delta = time.delta();
    recent_time.buffer.push_back(delta);
    if recent_time.buffer.len() >= window + 1 {
        recent_time.buffer.pop_front();
        let mut sorted: Vec<Duration> = recent_time.buffer.clone().into();
        sorted.sort_unstable();
        let sum = &sorted[2..(window - 2)].iter().fold(Duration::new(0, 0), |acc, x| acc + *x);
        smoothed_time.delta = *sum / (window as u32 - 4);
    } else {
        smoothed_time.delta = delta;
    }
}
