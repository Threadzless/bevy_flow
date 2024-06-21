//!

use std::{future::Future, sync::atomic::{AtomicU64, Ordering}};

use bevy::{ecs::system::{SystemParam, SystemState}, prelude::*, utils::hashbrown::HashMap};

use crate::{context::FlowContext, runner::{FlowTaskId, FlowTaskRunner}};


/// The [`SystemSet`] for when [`FlowTasksPlugin`] executes the 
/// next step in the **Flow** tasks.
#[derive(Clone, Debug, Hash, PartialEq, Eq, SystemSet)]
pub struct FlowTaskSystemSet;

/// Runs simple Flows asyncronously to the bevy Apps schedule.
/// 
/// **Flow**s are `async` functions which can request brief access to the
/// bevy Apps [`World`] object. This allows for complicated data processing
/// and action scheduling to be done without the complexities of multiple 
/// systems coordinated by [`State`]s and [`Event`]s.
/// 
/// Execution always takes place in the [`Update`] Schedule.
/// 
/// For timing control, see [`FlowTaskSystemSet`].
pub struct FlowTasksPlugin;

impl Plugin for FlowTasksPlugin {
        fn build(&self, app: &mut App) {
        app
            .init_state::<IsFlowing>()
            .init_resource::<FlowTaskList>()

            .add_systems(Update, 
                run_tasks.in_set(FlowTaskSystemSet)
            )
        ;
    }
}

#[derive(Clone, Debug, Default, Hash, PartialEq, Eq, States)]
enum IsFlowing {
    #[default]
    No,
    Yes,
}

/// All of the Flow Tasks that are in progress
#[derive(Default, Resource, Deref, DerefMut)]
pub struct FlowTaskList {
    #[deref]
    tasks: HashMap<FlowTaskId, FlowTaskRunner>,
    next_id: AtomicU64,
}

impl FlowTaskList {
    fn clean(&mut self) {
        self.tasks.retain(|_id, flow| !flow.is_finished())
    }

    fn next_id(&mut self) -> u64 {
        self.next_id.fetch_add(1, Ordering::Acquire)
    }
}


/// Mannage running flow tasks. See crate docs for what those are
#[derive(SystemParam)]
pub struct FlowTaskManager<'w, 's> {
    _cmds: Commands<'w, 's>,
    active: Res<'w, State<IsFlowing>>,
    next: ResMut<'w, NextState<IsFlowing>>,
    list: ResMut<'w, FlowTaskList>,
    assets: Option<Res<'w, AssetServer>>,
}

impl<'w, 's> FlowTaskManager<'w, 's> {
    /// Create and start a flow task.
    /// 
    /// ```rust
    /// # #![feature(async_closure)]
    /// # use bevy::{prelude::*, app::AppExit};
    /// # use bevy_flow::prelude::*;
    /// 
    /// #[derive(Clone, Copy, Debug, Default, Hash, PartialEq, Eq, States)]
    /// enum TerrainState {
    ///     #[default]
    ///     Generating,
    ///     Ready,
    /// }
    /// 
    /// #[derive(Resource)]
    /// struct MyTerrainResource;
    /// 
    /// impl MyTerrainResource {
    ///     fn new() -> Self {
    ///         Self { }
    ///     }
    /// 
    ///     // This will take a while
    ///     fn generate(&mut self) {
    ///         let mut count = 0;
    ///         for i in 0..10_000_000 {
    ///             count += (i % 4);
    ///         }
    ///         println!("Total: {count}");
    ///     }
    /// }
    /// 
    /// fn main() {
    ///     let mut app = App::new();
    ///     app
    ///         .init_state::<TerrainState>()
    ///         .add_plugins(MinimalPlugins)
    ///         .add_plugins(FlowTasksPlugin)
    ///         .add_systems(Startup, start_terrain_generation)
    ///         .add_systems(OnEnter(TerrainState::Ready), terrain_ready)
    ///         .run();
    /// }
    /// 
    /// fn start_terrain_generation(mut flow: FlowTaskManager) {
    ///     flow.start(do_terrain_generation);
    /// }
    /// 
    /// 
    /// async fn do_terrain_generation(mut ctx: FlowContext) {
    ///     // actions which don't use `ctx` will run independent
    ///     // of the bevy app, so you don't have to worry about blocking
    ///     let mut terrain = MyTerrainResource::new();
    ///     terrain.generate();
    /// 
    ///     // this will wait until the right time in the update cycle, and
    ///     // borrow access to `World` to accomplish the task.
    ///     ctx.insert_resource(terrain);
    /// 
    ///     // this won't happen until the update after the previous line
    ///     ctx.set_state(TerrainState::Ready);
    /// }
    /// 
    /// fn terrain_ready(mut exits: EventWriter<AppExit>) {
    ///     exits.send(AppExit);
    /// }
    /// ```
    pub fn start<Func, Fut>(&mut self, task_fn: Func) -> FlowTaskId
    where
        Func: FnOnce(FlowContext) -> Fut + Send + Sync + 'static,
        Fut: Future<Output=()> + Send + Sync,
    {
        let assets = self.assets.as_ref().map(|a| (*a).clone());
        let runner = FlowTaskRunner::new(task_fn, assets);
        let id = self.next_flow_task_id();

        let old = self.list.insert(id, runner);
        assert!(old.is_none());
        self.next.set(IsFlowing::Yes);
        id
    }

    /// Schedule a system to run exactly once in the [`Update`] Schedule
    /// 
    /// To ensure the it runs in the current [`Update`] cycle, schedule 
    /// the system that calls [`Self::soon`] before [`FlowTaskSystemSet`]. 
    /// 
    /// Scheduling after will delay running the provided system until the next 
    /// [`Update`] cycle
    /// 
    /// # Panics
    /// 
    /// While this method call will not panic, the thread it spawns will panic if:
    /// - One or more of the required resources is not present
    /// - A [`Component`] is requested by two or more [`Query`]s and at least one
    ///   of the requests is mutable without ensuring exclusivity
    /// - Any other reason a normal bevy system will panic
    pub fn soon<'a, Sys, M>(&mut self, _system: Sys) -> FlowTaskId
    where
        Sys: IntoSystem<(), (), M> + Send + Sync + 'static,
        // In: for<'w2, 's2> SystemParam::<State = (), Item<'w2, 's2>=In> + 'static
    {
        // self.start(async |ctx: FlowContext| {
        //     ctx.with(system);
        // })
        todo!()
    }

    fn next_flow_task_id(&mut self) -> FlowTaskId {
        let raw = self.list.next_id();
        warn!("FlowTask id={raw}");
        FlowTaskId(raw)
    }

    /// Returns the number of flows currently running. When a flow finishes
    /// execution it is cleaned up, and will no longer be counted.
    pub fn task_count(&self) -> usize {
        self.list.len()
    }

    /// Stop all running flow tasks.
    /// 
    /// This won't cause memory safety problems, but the threads are likely to panic.
    pub fn stop_all(&mut self) {
        for (_id, task) in self.list.drain() {
            drop(task);
        }
    }

    /// Returns `true` if there are any flow tasks currently
    /// running, `false` othersize
    pub fn are_any_running(&self) -> bool {
        match self.active.clone() {
            IsFlowing::No => false,
            IsFlowing::Yes => true,
        }
    }

    /// Returns an Iterator of all of the running FlowTasks
    pub fn iter(&self) -> impl Iterator<Item = (FlowTaskId, &FlowTaskRunner)> {
        self.list.iter()
            .map(|(id, t)| (*id, t))
    }

    /// Returns an mutable Iterator of all of the running FlowTasks
    pub fn iter_mut(&mut self) -> impl Iterator<Item = (FlowTaskId, &mut FlowTaskRunner)> {
        self.list.iter_mut()
            .map(|(id, t)| (*id, t)) 
    }

    /// Gets a [`FlowTaskRunner`] by its ID. If the task has finished or canceled,
    /// [`None`] will be returned
    pub fn get(&self, id: FlowTaskId) -> Option<&FlowTaskRunner> {
        self.list.get(&id)
    }

    /// Gets a [`FlowTaskRunner`] mutably by its ID. If the task has finished or canceled,
    /// [`None`] will be returned
    pub fn get_mut(&mut self, id: FlowTaskId) -> Option<&mut FlowTaskRunner> {
        self.list.get_mut(&id)
    }
}




fn run_tasks(
    world: &mut World,
    tasks: &mut SystemState<ResMut<FlowTaskList>>,
) {
    // this will be safe as long as the tasks internally don't try mutating
    // `FlowTaskList`, which they won't have access to as its private, 
    // so this should be safe
    let world_ref = unsafe { &mut *(world as *mut _) };

    let mut tasks = tasks.get_mut(world_ref);
    for (_id, task) in tasks.iter_mut() {
        task.loan_world(world);
    }

    tasks.clean();
    // if ! done.is_empty() { println!("ALL TASK DONE!!!") }
}