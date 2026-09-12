//! Raw guest pictures outlive native renderer entities. A patched tree carries
//! only hashes; mounting that tree in a new window still needs the first bytes.

use std::collections::HashMap;
use ui_lang_wire as wire;

const MAX_PICTURE_BYTES: usize = 8 * wire::MAX_PICTURE_BYTES_PER_FRAME;
const MAX_PICTURES: usize = 4096;

#[derive(Default)]
pub(super) struct Pictures {
    raster: HashMap<u64, wire::ImageData>,
    vector: HashMap<u64, Vec<u8>>,
}

impl Pictures {
    pub(super) fn adopt(&mut self, root: &mut wire::Node) {
        // Guests send each hash once. Eviction would lose an image the guest
        // still believes is present, so the lifetime budget refuses new hashes.
        let mut used = self.raster.values().map(wire::ImageData::byte_len).sum::<usize>()
            + self.vector.values().map(Vec::len).sum::<usize>();
        root.for_each_mut(&mut |node| match node {
            wire::Node::Image { hash, data: Some(data), .. }
            | wire::Node::ImageViewer { hash, data: Some(data), .. } => {
                let total = used + data.byte_len();
                let accepted = !self.raster.contains_key(hash) && total <= MAX_PICTURE_BYTES
                    && self.raster.len() + self.vector.len() < MAX_PICTURES;
                if accepted {
                    self.raster.insert(*hash, data.clone());
                    used = total;
                }
            }
            wire::Node::Svg { hash, bytes: Some(bytes), .. } => {
                let total = used + bytes.len();
                let accepted = !self.vector.contains_key(hash) && total <= MAX_PICTURE_BYTES
                    && self.raster.len() + self.vector.len() < MAX_PICTURES;
                if accepted {
                    self.vector.insert(*hash, bytes.clone());
                    used = total;
                }
            }
            _ => {}
        });
    }

    pub(super) fn hydrate(&self, root: &mut wire::Node) {
        root.for_each_mut(&mut |node| match node {
            wire::Node::Image { hash, data, .. } | wire::Node::ImageViewer { hash, data, .. } => {
                if data.is_none() { *data = self.raster.get(hash).cloned(); }
            }
            wire::Node::Svg { hash, bytes, .. } => {
                if bytes.is_none() { *bytes = self.vector.get(hash).cloned(); }
            }
            _ => {}
        });
    }
}
