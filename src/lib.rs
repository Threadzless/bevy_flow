#![doc = include_str!("../README.md")]

#![warn(missing_docs)]
#![warn(clippy::missing_panics_doc)]
#![warn(clippy::absolute_paths)]

#![feature(async_closure)]
#![feature(exact_size_is_empty)]

mod context;
mod plugin;
mod runner;

pub use plugin::{FlowTasksPlugin, FlowTaskManager};
pub use context::{FlowContext, WorldRef};

// impl<E> On<E>
// pub fn run<Marker>(callback: impl IntoSystem<(), (), Marker>) -> Self
// where
//     // Bounds from impl:
//     E: EntityEvent,