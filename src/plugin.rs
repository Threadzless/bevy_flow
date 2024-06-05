use std::future::Future;

use bevy::{ecs::system::{SystemParam, SystemState}, prelude::*};

use crate::{context::FlowContext, runner::FlowTaskRunner};


/// Runs 
pub struct FlowTasksPlugin;

// impl<S: ScheduleLabel + Clone> FlowTasksPlugin<S> {
//     /// Create a [`FlowTasksPlugin`] where the flow task exclusive system
//     /// runs on a schedule of your choosing.
//     pub fn new(schedule: S) -> Self {
//         Self { schedule }
//     }
// }

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
            .add_systems(Update, run_tasks)
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

    // if tasks_list.is_empty() { return }

    for t in tasks.get_mut(world_ref).iter_mut() {
        t.loan_world(world);
        // if ! t.loan_world(world) {
        //     done.push(index);
        // }
    }

    // if ! done.is_empty() { println!("ALL TASK DONE!!!") }
}