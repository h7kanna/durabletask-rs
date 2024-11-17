// Taken from https://github.com/temporalio/sdk-core/blob/master/sdk/src/app_data.rs
use std::{
    any::{Any, TypeId},
    collections::HashMap,
    fmt,
};

/// A Wrapper Type for orchestrator and activity app data
#[derive(Default)]
pub(crate) struct Environment {
    map: HashMap<TypeId, Box<dyn Any + Send + Sync>>,
}

impl Environment {
    /// Insert an item, overwriting duplicates
    pub(crate) fn insert<T: Send + Sync + 'static>(&mut self, val: T) -> Option<T> {
        self.map
            .insert(TypeId::of::<T>(), Box::new(val))
            .and_then(downcast_owned)
    }

    /// Get a reference to a type in the map
    pub(crate) fn get<T: 'static>(&self) -> Option<&T> {
        self.map
            .get(&TypeId::of::<T>())
            .and_then(|boxed| boxed.downcast_ref())
    }
}

impl fmt::Debug for Environment {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("Environment").finish()
    }
}

fn downcast_owned<T: Send + Sync + 'static>(boxed: Box<dyn Any + Send + Sync>) -> Option<T> {
    boxed.downcast().ok().map(|boxed| *boxed)
}
