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
use bevy::prelude::*;
use bevy_flow::prelude::*;

fn main() {
    let mut app = App::new()
        .add_plugins(DefaultPlugins)
        .add_plugins(FlowingTaskPlugin);

        .add_systems(Startup, generate_world)

    app.run()
}

fn generate_world(mut flows: FlowingTaskManager) {
    flows.start(async |mut access: FlowAccess|{
        // ensure all of these assets are fully loaded before proceeding
        let folder = access.load_folder("images/terrain").await;

        // combine those images into a Texture2dArray
        let mut texture_atlas = TextureAtlasBuilder::default();
        access.for_each_image(|image: &Image, id: AssetId<Image>, path: Option<PathBuf>| {
            texture_atlas.add_texture(Some(id), image);
        }).await;

        let (_layout, mut tile_sheet) = self.builder.finish().unwrap();
        access.
    });
}
```

## TODO

- Try and switch from using threads to tasks
- Remove this crate from the workspace and make it its own thing. Maybe called bevy_flow_tasks or something
