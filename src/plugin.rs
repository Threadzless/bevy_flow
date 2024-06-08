use std::future::Future;

use bevy::{ecs::system::{SystemParam, SystemParamItem, SystemState}, prelude::*};

use crate::{context::FlowContext, runner::FlowTaskRunner};


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


impl Default for FlowTasksPlugin {
    fn default() -> Self {
        Self
    }
}

// impl<S: ScheduleLabel + Clone> Plugin for FlowTasksPlugin<S> {
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

#[derive(Default, Resource, Deref, DerefMut)]
pub struct FlowTaskList(Vec<FlowTaskRunner>);

impl FlowTaskList {
    fn clean(&mut self) {
        self.0.retain(|flow| flow.is_in_progress())
    }
}


/// Mannage running flow tasks. See crate docs for what those are
#[derive(SystemParam)]
pub struct FlowTaskManager<'w, 's> {
    _cmds: Commands<'w, 's>,
    active: Res<'w, State<IsFlowing>>,
    next: ResMut<'w, NextState<IsFlowing>>,
    list: ResMut<'w, FlowTaskList>,
    assets: Res<'w, AssetServer>,
}

impl<'w, 's> FlowTaskManager<'w, 's> {
    /// Create and start a flow task
    pub fn start<Func, Fut>(&mut self, task_fn: Func) 
    where
        Func: FnOnce(FlowContext) -> Fut + Send + Sync + 'static,
        Fut: Future<Output=()> + Send + Sync,
    {
        let runner = FlowTaskRunner::new(task_fn, self.assets.clone());

        self.list.push(runner);
        self.next.set(IsFlowing::Yes);
    }

    /// Returns the number of flows currently running. When a flow finishes
    /// execution it is cleaned up, and will no longer be counted.
    pub fn task_count(&self) -> usize {
        self.list.len()
    }

    /// Schedules a task to execute one in the next [`FlowTaskSystemSet`] that 
    /// will have mutable access to a single [`Resource`].
    pub fn use_resource_soon<R: Resource, F>(&mut self, task_fn: F)
    where
        F: Fn(&mut R) -> () + Send + Sync + 'static
    {
        self.start(async |ctx: FlowContext| {
            ctx.with_resource::<R, ()>(task_fn)
        });
    }

    ///
    pub fn use_system_param<P: SystemParam>(
        &mut self,
        task_fn: impl Fn(SystemParamItem<P>) -> () + Send + Sync + 'static
    )
    where
        P: SystemParam + 'static,
    {
        self.start(async move |ctx: FlowContext| {
            ctx.with::<P, _>(task_fn);
        })
    }

    /// Immediatly stop all running flow tasks. 
    /// 
    /// This won't cause memory safety problems
    pub fn stop_all(&mut self) {
        for t in self.list.drain(..) {
            t.cancel();
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

    /// pause all flow tasks the next time they need to access
    /// 
    /// **Warning:** If a system is paused while a state change or event
    /// it is waiting on occures, it will be missed. This could cause the
    /// flow to wait forever, so use this feature cautiously.
    pub fn pause(&mut self) {
        self.next.set(IsFlowing::No);
    }

    /// opposite of pause
    pub fn resume(&mut self) {
        self.next.set(IsFlowing::Yes)
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
    for t in tasks.iter_mut() {
        t.loan_world(world);
    }

    tasks.clean();
    // if ! done.is_empty() { println!("ALL TASK DONE!!!") }
}