//! 

use std::{future::Future, thread::{JoinHandle, spawn}};

use bevy::{prelude::*, tasks::futures_lite::future::block_on};
use async_channel::{bounded, Receiver, Sender};

use crate::context::FlowContext;


/// A unique id to track a 
#[derive(Clone, Copy, Debug, Hash, PartialEq, Eq)]
pub struct FlowTaskId(pub(crate) u64);

/// Manages the execution of a flow task
pub struct FlowTaskRunner {
    send: Sender<LTMsg>,
    recv: Receiver<LTResult>,
    task: JoinHandle<()>
}

// unsafe impl Send for FlowTaskRunner { }
// unsafe impl Sync for FlowTaskRunner { }

impl FlowTaskRunner {

    /// Start a new long running task. It will start immediatly
    pub fn new<Func, Fut>(task_fn: Func, assets: Option<AssetServer>) -> Self 
    where
        Func: FnOnce(FlowContext) -> Fut + Send + Sync + 'static,
        Fut: Future<Output=()> + Send + Sync,
    {
        let (send, recv_far) = bounded(5);
        let (send_far, recv) = bounded(5);
            
        let task = spawn(move || {
            block_on(async {
                let send_done = send_far.clone();
                let tasker = FlowContext::new(send_far, recv_far, assets);
                task_fn(tasker).await;

                send_done.send(LTResult::Finished).await.unwrap();
            });
        });

        Self {
            send,
            recv,
            task,
        }
    }

    /// Loan the [`World`] object to this task for a moment.
    /// 
    /// This is done automatically by 
    pub fn loan_world(&mut self, world: &mut World) -> bool {
        if self.recv.is_empty() { return false }

        block_on( self.load_world_call(world) )
    }

    /// Returns `true` if the task has completed.
    /// 
    /// [`FlowTasksPlugin`]
    pub fn is_finished(&self) -> bool {
        self.task.is_finished()
    }

    async fn load_world_call(&self, world: &mut World) -> bool {
        match self.recv.recv().await {
            Ok(LTResult::RequestingWorld) => {
                let msg = LTMsg::World(world as *mut _);

                if let Err(err) = self.send.send(msg).await {
                    panic!("Load World Send: {err:?}");
                }
    
                match self.recv.recv().await {
                    Ok(LTResult::DoneWithWorld) => { 
                        return false;
                    },
                    Ok(_) => println!("Load World Recv Bad"),
                    Err(err) => panic!("Load World Recv: {err:?}"),
                }
    
                false
            },
            Ok(LTResult::Finished) => true,
            Ok(_) => todo!(),
            Err(err) => todo!("Err: {err:?}"),
        }
    }
}






pub(crate) enum LTMsg {
    World(*mut World),
}

unsafe impl Send for LTMsg { }
unsafe impl Sync for LTMsg { }

pub(crate) enum LTResult {
    DoneWithWorld,
    RequestingWorld,
    Finished,
}