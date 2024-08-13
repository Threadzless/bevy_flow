#![feature(async_closure)]

use std::ops::Add;

use bevy::{app::AppExit, prelude::*};
use bevy_flow::prelude::*;


#[derive(Resource, Default)]
struct UpdateTimesList {
    pub times: Vec<f32>,
}

#[derive(Event)]
struct TaskComplete;

#[derive(Clone, Debug, Default, Hash, PartialEq, Eq, States)]
enum ToggleableState {
    #[default]
    A,
    B,
    C
}



fn main() {
    let mut app = App::new();
    app.add_plugins(DefaultPlugins);
    app.add_plugins(FlowTasksPlugin);

    app.add_event::<TaskComplete>();
    app.init_state::<ToggleableState>();
    app.init_resource::<UpdateTimesList>();

    app.add_systems(Startup, start_task);
    app.add_systems(Update, track_cycles);
    app.add_systems(PostUpdate, complete);

    app.run();
}



fn start_task(mut tasks: FlowTaskManager) {
    tasks.start(async |mut ctx: FlowContext| {
        info!("Flow Task Started");

        ctx.set_state(ToggleableState::A);
        info!("ToggleableState == A");

        ctx.set_state(ToggleableState::B);
        info!("ToggleableState == B");

        ctx.set_state(ToggleableState::C);
        info!("ToggleableState == C");

        ctx.send_event(TaskComplete);
        info!("Task Complete!");
    });
}



fn track_cycles(time: Res<Time>, mut utl: ResMut<UpdateTimesList>) {
    utl.times.push(time.elapsed_seconds())
}



fn complete(
    utl: Res<UpdateTimesList>,
    mut exit: EventWriter<AppExit>,
    mut events: EventReader<TaskComplete>,
) {
     // only move forward if there is a `TaskComplete` event
     if let Some(_evt) = events.read().next() {
        let ms = utl.times.iter()
            .map(|sec| (sec * 1000.0) as u32 )
            .collect::<Vec<_>>();

        println!("Timing Statistics:");
        println!("  Cycles:       {}", utl.times.len());
        let total_time: f32 = utl.times.iter()
            .copied()
            .fold(0.0, Add::add);
        println!("  Total Time:   {} seconds", total_time);
        println!("  Average Time: {} seconds", total_time / utl.times.len() as f32);
        print!(  "  Cycle times:\n\t");
        for v in ms {
            print!("{v}ms\t");
        }
        print!("\n");

        exit.send(AppExit::Success);
    }
}