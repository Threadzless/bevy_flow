#![doc = include_str!("../README.md")]

#![warn(missing_docs)]
#![warn(clippy::missing_panics_doc)]
#![warn(clippy::absolute_paths)]

#![feature(async_closure)]
#![feature(unboxed_closures)]
#![feature(exact_size_is_empty)]

pub mod context;
pub mod plugin;
pub mod runner;

/// The stuff you will likely need, all in one place
pub mod prelude {
    pub use crate::context::{FlowContext, WorldRef};
    pub use crate::plugin::{FlowTasksPlugin, FlowTaskSystemSet, FlowTaskManager};
}
