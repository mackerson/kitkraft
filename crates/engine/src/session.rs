//! The state one front end keeps between commands: the loaded load order and caches.
//!
//! A [`crate::Engine`] owns one [`Session`]. Session-aware handlers
//! ([`crate::SessionHandler`]) read and change it; stateless handlers
//! ([`crate::CommandHandler`]) never see it. The free [`crate::execute`] runs every call
//! on a fresh, empty session, so the CLI (one command per process) behaves as before.
//!
//! The session does not name the types it holds: lanes that own a piece of state (the
//! load order from `crates/doc`, string tables, index caches) store it as a typed slot
//! with [`Session::insert`] and read it back with [`Session::get`], so adding state never
//! edits this file. The one thing every front end reads without knowing those types is
//! [`Session::load_order`]: the JSON listing of the current load order that the `load`
//! command produces and the MCP `kitkraft://load-order` resource serves.

use std::any::{Any, TypeId};
use std::collections::BTreeMap;

use serde_json::Value;

/// State kept across commands on one engine. See the module docs.
#[derive(Default)]
pub struct Session {
    load_order: Option<Value>,
    slots: BTreeMap<TypeId, Box<dyn Any + Send>>,
}

impl std::fmt::Debug for Session {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Session")
            .field("load_order", &self.load_order.is_some())
            .field("slots", &self.slots.len())
            .finish()
    }
}

impl Session {
    /// An empty session: no load order, no cached state.
    pub fn new() -> Self {
        Self::default()
    }

    /// The JSON listing of the current load order (plugins, masters, indices), as the last
    /// `load` reported it; `None` before any load.
    pub fn load_order(&self) -> Option<&Value> {
        self.load_order.as_ref()
    }

    /// Replaces the load-order listing. Handlers that change the load order keep it in
    /// step with the typed state they store in a slot.
    pub fn set_load_order(&mut self, listing: Option<Value>) {
        self.load_order = listing;
    }

    /// The slot of type `T`, if one was stored.
    pub fn get<T: Any + Send>(&self) -> Option<&T> {
        self.slots
            .get(&TypeId::of::<T>())
            .and_then(|slot| slot.downcast_ref::<T>())
    }

    /// The slot of type `T`, mutably.
    pub fn get_mut<T: Any + Send>(&mut self) -> Option<&mut T> {
        self.slots
            .get_mut(&TypeId::of::<T>())
            .and_then(|slot| slot.downcast_mut::<T>())
    }

    /// Stores `value` in the slot of type `T`, returning the previous one.
    pub fn insert<T: Any + Send>(&mut self, value: T) -> Option<T> {
        self.slots
            .insert(TypeId::of::<T>(), Box::new(value))
            .and_then(|old| old.downcast::<T>().ok())
            .map(|old| *old)
    }

    /// Removes and returns the slot of type `T`.
    pub fn remove<T: Any + Send>(&mut self) -> Option<T> {
        self.slots
            .remove(&TypeId::of::<T>())
            .and_then(|old| old.downcast::<T>().ok())
            .map(|old| *old)
    }

    /// Drops the load order and every slot.
    pub fn clear(&mut self) {
        self.load_order = None;
        self.slots.clear();
    }

    /// Whether nothing is loaded or cached.
    pub fn is_empty(&self) -> bool {
        self.load_order.is_none() && self.slots.is_empty()
    }
}
