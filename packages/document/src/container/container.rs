use std::collections::HashMap;

use serde::{Deserialize, Serialize};

#[derive(Debug)]
pub(crate) struct Container {
    // head: Node,
    node_map: HashMap<uid::Uid, nodes::Nodes>

    
}

impl Container {
    pub fn insert_after(&mut self, uid: uid::Uid) -> Result<(), ()> {
        Ok(())
    }    

    pub fn remove(&mut self, uid: uid::Uid) -> Result<(), ()> {
        Ok(())
    }
}

impl Serialize for Container {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: serde::Serializer {
        todo!()
    }
}

impl <'de>Deserialize<'de> for Container {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: serde::Deserializer<'de> {
        todo!()
    }
}
