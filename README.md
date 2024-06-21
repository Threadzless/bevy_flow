# bevy_flow

## Problem

For games made with [`bevy`](https://github.com/bevyengine/bevy), it can be difficult to program complicated, cpu or I/O intensive tasks that involve data for many places at once. Loading and processing content that can be altered by users, such as for creating mods, is a great example of this problem.

Programers have to compromise between the game freezing while these long tasks run, or splitting the actions of these tasks into different systems, which need to be coordinated with `State`s, `Event`s and `Resource`s.

Neither is ideal

## Solution

**`bevy_flow`** solves this by making one big function, or `Flow`, and giving that function access to resources on an as-needed basis.

This is achieved by periodically passing a pointer to bevy's `World` object to the `Flow` on another thread for a moment, and then taking it back so the app isn't halted while the `Flow` runs.

### Pros

**Easy** - Complicated `Flow`s are far easer to write, manage, and update, as they don't require juggling `State`s, `Resource`s, or `Event`s to maintain order.

**Safe** - Doesn't violate rust's memory safety, there's barely any `unsafe` code, and thats just to keep the borrow checker happy.

### Cons

**Exclusive** - `Flow`s require use of bevy's [Exclusive System](https://bevy-cheatbook.github.io/programming/exclusive.html) while they are running. Exclusive systems _can_ easily become performance bottlenecks, because no other systems can run at the same time. `bevy_flow` makes great efforts to minimize this downside

**Threaded** - Each `Flow` runs on its own thread. Threads have far more overhead than Task and systems, so you should limit how many of these you have at a time. Using more than one hasn't been tested, so if you need lots of these, I recomend you look at alternatives:

- [`bevy-sequential-actions`](https://crates.io/crates/bevy-sequential-actions)
- [`bevy_async_task`](https://crates.io/crates/bevy-async-task)

## Ideal Use cases

- **Asset Processing** Assets can be loaded, read, altered, and generated, with minimal little interfearing with the main apps schedule

- **Networking and Multiplayer** Sometimes you need to send a bunch of data over the network before someone can join a server

- **Other things that rely on I/O** ¯\_(ツ)_/¯

## Example

```rust
#![feature(async_closure)]
use bevy::{prelude::*, app::AppExit};
use bevy_flow::prelude::*;

fn main() {
    let mut app = App::new();
    app
        .init_state::<TerrainState>()
        .add_plugins(MinimalPlugins)
        .add_plugins(FlowTasksPlugin)
        .add_systems(OnEnter(TerrainState::Ready), exit_when_terrain_is_ready)
        .add_systems(Startup, |mut flow: FlowTaskManager| {
            flow.start(do_terrain_generation);
        });

    app.run();
}

/// This is the FlowTask. It will run in parallel to the bevy app
async fn do_terrain_generation(mut ctx: FlowContext) {
    // actions which don't use `ctx` will run independent
    // of the bevy app, so you don't have to worry about blocking
    let mut terrain = MyTerrainResource::new();
    terrain.generate();

    // this will wait until the right time in the update cycle, and
    // borrow access to `World` to accomplish the task.
    ctx.insert_resource(terrain);

    // this won't happen until the update after the previous line
    ctx.set_state(TerrainState::Ready);

    // borrow the [`World`] from the bevy app at the next opportunity, 
    // through a reference
    let world_ref = ctx.borrow();

    // ... do stuff ... //

    // when the reference is dropped, the [`World`] is returned to
    // the bevy app for at least one [`Update`] cycle
    drop(world_ref);
}


#[derive(Clone, Copy, Debug, Default, Hash, PartialEq, Eq, States)]
enum TerrainState {
    #[default]
    Generating,
    Ready,
}

#[derive(Resource)]
struct MyTerrainResource;

impl MyTerrainResource {
    fn new() -> Self {
        Self { }
    }

    // This will take a while
    fn generate(&mut self) {
        let mut count = 0;
        for i in 0..10_000_000 {
            count += (i % 4);
        }
        println!("Total: {count}");
    }
}

fn exit_when_terrain_is_ready(mut exits: EventWriter<AppExit>) {
    exits.send(AppExit);
}
```

## TODO

- Try and switch from using threads to tasks
- Remove this crate from the workspace and make it its own thing. Maybe called bevy_flow_tasks or something
