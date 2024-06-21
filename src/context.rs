//!

use std::{any::type_name, ops::{Deref, DerefMut}};

use bevy::{
    asset::{AssetPath, LoadedFolder},
    ecs::{event::EventId, system::{SystemParam, SystemState}},
    prelude::*,
    tasks::block_on
};
use async_channel::{Receiver, Sender};

use crate::runner::{LTMsg, LTResult};







/// Provides safe access to a bevy [`World`] in the context of
/// 
pub struct FlowContext {
    send: Sender<LTResult>,
    recv: Receiver<LTMsg>,
    assets: Option<AssetServer>,
}

impl FlowContext {
    pub(crate) fn new(
        send: Sender<LTResult>,
        recv: Receiver<LTMsg>,
        assets: Option<AssetServer>
    ) -> Self {
        Self {
            send,
            recv,
            assets,
        }
    }

    async fn request_world(&self) -> *mut World {
        if let Err(err) = self.send.send(LTResult::RequestingWorld).await {
            panic!("LongTaskRunner must have dropped {err:?}");
        }

        match self.recv.recv().await {
            Ok(LTMsg::World(world_ptr)) => world_ptr,
            Err(err) => panic!("{err:?}")
        }
    }

    fn world_sync(&self) -> WorldRef<'_> {
        block_on(self.borrow())
    }
}


impl FlowContext {
    /// Directly borrow bevy's [`World`]. This is the most powerful, but
    /// inellegant way to do this.
    /// 
    /// What will actually be returned is a [`WorldRef`], which derefs into
    /// a [`World`], and when it's dropped, the `World` is sent back to the main
    /// app.
    /// 
    /// While this reference is held, the rest of the bevy app is halted, so be sure
    /// to periodically drop it and borrow again to prevent the main app from stuttering
    pub async fn borrow(&self) -> WorldRef<'_> {
        let world_ptr = self.request_world().await;
        WorldRef {
            world: unsafe { &mut *world_ptr },
            linker: self,
        }
    }

    /// Directly use the [`World`]. While this function is running, the rest of 
    /// your bevy App is halted by an exclusive system, so don't do too much in one
    /// of these.
    /// 
    /// # Panics
    /// 
    /// Panics if the controling [`FlowTaskRunner`](super::runner::FlowTaskRunner) 
    /// is dropped. This shouldn't happen
    pub fn with_world<Ret>(&self, call: impl FnOnce(&mut World) -> Ret) -> Ret {
        block_on(async {
            let world_ptr = self.request_world().await;
            let world = unsafe { &mut *world_ptr };

            let ret = call(world);
            self.send.send(LTResult::DoneWithWorld).await.unwrap();
            ret
        })
    }

    /// Run a system once. This works similar to bevy's [`App::add_systems`].
    /// The main difference is the provided callback is only runs once, at this point
    /// in the flow. 
    /// 
    /// also the passed system can return a value to the earlier context
    /// 
    /// ### Example
    /// ```ignore
    /// # #![feature(async_closure)]
    /// # use bevy::prelude::*;
    /// # use bevy_flow::prelude::*;
    /// 
    /// flow.start(async |mut ctx: FlowContext| {
    ///     // Use bevy resources, just like a system
    ///     let ret = ctx.once(|mut events: EventReader<CursorMoved>, mut cmds: Commands| {
    ///         // do stuff with resources
    ///         32 // return value to pass to parent flow scope
    ///     });
    ///     assert_eq!(ret, 32);
    /// })
    /// ```
    /// # 
    /// The callback can contain 0-16 arguemnts, which must implement [`SystemParam`]. 
    /// This includes:
    /// - [`Commands`]
    /// - [`Query`]s
    /// - Any [`Resource`], using [`Res`] or [`ResMut`].
    /// - [`EventReader`]s and [`EventWriter`]s.
    /// - Optional `Resources` (`Option<Res<...>>` or `Option<ResMut<...>>`)
    /// - Tuples with upto 16 elements where all the elements implement [`SystemParam`]. This
    ///   works recursively, so there is no hard limit on how many parameters you can bring
    ///   into scope at once.
    /// 
    /// **Note**: [`Local`]s will work, but as thsi system runs exactly once, using them 
    /// is pointless
    /// 
    /// This doesn't support exclusive systems, so if you need to access 
    /// [`World`], use [`with_world`](Self::with_world). 
    /// 
    /// # Panics
    /// 
    /// This for the same reasons a normal bevy system would panic:
    /// - Two or more `Query`s request access to the same [`Component`], and at least one 
    ///   of them are mutable.
    /// - A `Resource`, [`Event`], or [`State`] is requested that isn't present.
    pub fn with<'a, Sys, Out, Params>(&self, _system: Sys) -> Out
    where
        Params: SystemParam + 'static,
        Sys: FnOnce(Params::Item<'a, 'a>) -> Out + 'a,
        // Sys: SystemParamFunction<()> + FnOnce(Params) -> Out + 'a,
        Out: Send + Sync + 'a
    {
        // Sys::Param::
        self.with_world(|world| {
            let mut state = SystemState::<Params>::new(world);
            let state_ref = unsafe {
                &mut *(&mut state as *mut SystemState::<Params>)
            };
            let params = state_ref.get_mut(unsafe { &mut *(world as *mut _)});
            let out = _system(params);
            let state_ref = unsafe {
                &mut *(&mut state as *mut SystemState::<Params>)
            };
            state_ref.apply(world);
            out
        })
    }

    /// Gets a copy of a [`Resource`]
    /// 
    /// # Panics
    /// 
    /// Panics if the Resource doesn't exist
    pub fn copy_resource<R>(&self) -> R
    where
        R: Resource + Clone 
    {
        self.with_world(|world| {
            world.get_resource::<R>().unwrap().clone()
        })
    }

    /// Inserts a new resource with the given value.
    /// 
    /// Resources are "unique" data of a given type. If you insert a 
    /// resource of a type that already exists, you will overwrite any existing data.
    pub fn insert_resource<R>(&self, resource: R) 
    where 
        R: Resource 
    {
        self.with_world(|world| {
            world.insert_resource(resource)
        })
    }

    /// Directly access the [`AssetServer`]. Using this doesn't effect the main apps 
    /// scheduling, and will work as normal
    /// 
    /// # Panics
    /// 
    /// Panics if the [`AssetPlugin`] is not available
    pub fn asset_server(&self) -> &AssetServer {
        self.assets.as_ref().unwrap()
    }

    /// Same as [`AssetServer::load`]
    /// 
    /// # Panics
    /// 
    /// Panics if the [`AssetPlugin`] is not available
    pub fn load_asset<'a, A: Asset>(&self, path: impl Into<AssetPath<'a>>) -> Handle<A> {
        let assets = self.assets.as_ref().unwrap();
        assets.load(path)
    }

    /// Same as [`AssetServer::load_folder`]
    /// 
    /// # Panics
    /// 
    /// Panics if the [`AssetPlugin`] is not available
    pub fn load_folder<'a>(&self, path: impl Into<AssetPath<'a>>) -> Handle<LoadedFolder> {
        let assets = self.assets.as_ref().unwrap();
        assets.load_folder(path)
    }

    /// Schedules changing a [`State`] resource at the end of the next update cycle.
    /// 
    /// This is equivalent to calling [`NextState::set`] in a normal system
    /// 
    /// If the state is not present in the app it is added.
    pub fn set_state<S: States>(&self, new: S) {
        let mut world = self.world_sync();
        if let Some(mut next) = world.get_resource_mut::<NextState<S>>() {
            next.set(new);
        }
        else {
            world.insert_resource(State::new(new.clone()));
            world.insert_resource(NextState::<S>::default())
        }
    }

    /// Sends an [`Event`] to the game, that will be recieved on the next update cycle.
    /// 
    /// This is the same as calling [`EventWriter::send`] in a normal system
    /// 
    /// # Panics
    /// 
    /// Panics if the the event hasn't been insterted into the bevy App.
    /// 
    /// See [`App::add_event`]
    pub fn send_event<E: Event>(&mut self, event: E) -> EventId<E> {
        let mut world = self.world_sync();
        let mut events = world.get_resource_mut::<Events<E>>().unwrap();
        events.send(event)
    }

    /// Get the current state
    /// 
    /// # Panics
    /// 
    /// Panics if the the State hasn't been insterted into the bevy App.
    /// 
    /// See [`App::init_state`] or [`App::insert_state`]
    pub fn get_state<S: States>(&self) -> S {
        let world = self.world_sync();
        let next = world.get_resource::<State<S>>().unwrap();
        next.get().clone()
    }



    /// Loads a folder, like [`AssetServer::load_folder`], then waits until the 
    /// every file in that folder is loaded, then returns a list of all
    /// the assets loaded loaded
    /// 
    /// **NOTE:** Be sure to use `await` on this function or it will be skipped
    /// 
    /// # Panics
    /// 
    /// Panics if the [`AssetPlugin`] is not available
    #[allow(clippy::missing_panics_doc)] // shouldn't be able to panic
    pub async fn await_folder(
        &self, 
        path: impl Into<AssetPath<'_>>
    ) -> (Handle<LoadedFolder>, LoadedFolder) 
    {
        let assets = self.asset_server();
        let folder_handle = assets.load_folder(path);
        let folder_id = Into::<AssetId<LoadedFolder>>::into(folder_handle.clone());
        
        let _ = self.await_event::<AssetEvent<LoadedFolder>>(|evt| {
            match evt {
                AssetEvent::LoadedWithDependencies { id } => &folder_id == id,
                _ => false
            }
        }).await;

        let world = self.borrow().await;
        let folders = world.get_resource::<Assets<LoadedFolder>>().unwrap();

        let folder = folders.get(folder_handle.clone()).unwrap();
        let folder = LoadedFolder {
            handles: folder.handles.clone(),
        };
        (folder_handle, folder)
    }

    /// Wait until an event which satisfies `filter` occures before continuing
    /// 
    /// **NOTE:** Be sure to use `await` on this function or it will be skipped
    ///
    /// # Panics
    /// 
    /// Panics if the the event hasn't been insterted into the bevy App.
    /// 
    /// See [`App::add_event`]
    pub async fn await_event<E>(&self, filter: impl Fn(&E) -> bool) 
    where
        E: Event
    {
        loop {
            let world = self.borrow().await;
            let Some(events) = world.get_resource::<Events<E>>() else {
                panic!("Resource {} is not present", type_name::<E>())
            };
            let mut reader = events.get_reader();

            for evt in reader.read(events) {
                if filter(evt) { return }
            }
        }
    }
    
    /// Wait until an event which satisfies `filter` occures before returning it.
    /// This requires the event to implement [`Clone`]
    /// 
    /// **NOTE:** Be sure to use `await` on this function or it will be skipped
    ///
    /// # Panics
    /// 
    /// Panics if the the event hasn't been insterted into the bevy App.
    /// 
    /// See [`App::add_event`]
    pub async fn await_event_return<E, Ret>(&self, filter: impl Fn(&E) -> Option<Ret>) -> Ret
    where
        E: Event
    {
        loop {
            let world = self.borrow().await;
            let Some(events) = world.get_resource::<Events<E>>() else {
                panic!("Flow Context unable to locate event: {}", type_name::<E>());
            };
            for evt in events.get_reader().read(events) {
                if let Some(ret) = filter(evt) {
                    return ret
                }
            }
        }
    }

    /// Delay this flow until a given state is reached.
    /// 
    /// **NOTE:** Be sure to use `await` on this function or it will be skipped
    ///
    /// # Panics
    /// 
    /// Panics if the the State hasn't been insterted into the bevy App.
    /// 
    /// See [`App::init_state`] or [`App::insert_state`]
    pub async fn await_state<S: States>(&self, matches: S) {
        loop {
            let world = self.borrow().await;
            let state = world.get_resource::<State<S>>().unwrap();
            if state.get() == &matches {
                return
            }
        }
    }

    /// Wait until a certain condition is met before continuing
    /// 
    /// **NOTE:** Be sure to use `await` on this function or it will be skipped
    pub async fn await_cond<R>(&self, cond: impl Fn(&WorldRef) -> Option<R>) -> R {
        loop {
            let world = self.borrow().await;
            if let Some(ret) = cond(&world) {
                return ret
            }
        }
    }
}





/// Temporary access to bevy's [`World`] across threads safely.
/// 
/// When this struct is dropped, the [`TaskAccess`] that created it
/// will return ownership of the `World` back to bevy.
pub struct WorldRef<'a> {
    world: &'a mut World,
    linker: &'a FlowContext,
}

impl<'a> Drop for WorldRef<'a> {
    fn drop(&mut self) {
        block_on({
            self.linker.send.send(LTResult::DoneWithWorld)
        }).unwrap();
    }
}

impl<'a> Deref for WorldRef<'a> {
    type Target = World;
    fn deref(&self) -> &Self::Target {
        self.world
    }
}

impl<'a> DerefMut for WorldRef<'a> {
    fn deref_mut(&mut self) -> &mut Self::Target {
        self.world
    }
}

impl<'a> WorldRef<'a> {
    // /// Schedules changing a [`State`] resource at the end of the next update cycle.
    // /// 
    // /// This is equivalent to calling [`NextState::set`] in a normal system
    // /// 
    // /// # Panics
    // /// 
    // /// Panics if the the State hasn't been insterted into the bevy App.
    // /// 
    // /// See [`App::init_state`] or [`App::insert_state`]
    // pub fn set_state<S: States>(&mut self, new: S) {
    //     println!("Setting State {new:?}");
    //     let mut next = self.world.get_resource_mut::<NextState<S>>().unwrap();
    //     next.set(new);
    // }

    // /// Sends an [`Event`] to the game, that will be recieved on the next update cycle.
    // /// 
    // /// This is the same as calling [`EventWriter::send`] in a normal system
    // /// 
    // /// # Panics
    // /// 
    // /// Panics if the the event hasn't been insterted into the bevy App.
    // /// 
    // /// See [`App::add_event`]
    // pub fn send_event<E: Event>(&mut self, event: E) -> EventId<E> {
    //     println!("Sending Event");
    //     let mut events = self.world.get_resource_mut::<Events<E>>().unwrap();
    //     events.send(event)
    // }

    // /// Get the current state
    // /// 
    // /// # Panics
    // /// 
    // /// Panics if the State is not present
    // /// 
    // /// See [`App::init_state`] or [`App::insert_state`]
    // pub fn get_state<S: States>(&self) -> S {
    //     let next = self.world.get_resource::<State<S>>().unwrap();
    //     next.get().clone()
    // }
}


// pub struct ThingOwn<'a, T: 'a, O: 'a> {
//     world: WorldRef<'a>,
//     thing: T,
//     other: O,
// }



// impl<'a, T: 'a, O: 'a> ThingOwn<'a, T, O> {
//     #[allow(unused)]
//     fn from_world(world: WorldRef<'a>, thing: T, other: O) -> ThingOwn<'a, T, O> {
//         Self { world, thing, other }
//     }

//     fn new(thing: T, other: O, world_ref: WorldRef<'a>) -> Self {
//         Self {
//             world: world_ref,
//             thing,
//             other,
//         }
//     }

//     pub fn back_to_world(self) -> WorldRef<'a> {
//         self.world
//     }
// }

// impl<'a, T, O> Deref for ThingOwn<'a, T, O> {
//     type Target = T;
//     fn deref(&self) -> &Self::Target {
//         &self.thing
//     }
// }

// impl<'a, T, O> DerefMut for ThingOwn<'a, T, O> {
//     fn deref_mut(&mut self) -> &mut Self::Target {
//         &mut self.thing
//     }
// }


