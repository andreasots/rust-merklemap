#![crate_id = "merklemap#0.1"]
#![crate_type = "lib"]

use std::io::{IoResult, SeekSet};
use std::default::Default;
use std::slice::bytes::copy_memory;
use std::vec::Vec;

static KEY_BYTES: uint = 32;
static HASH_BYTES: uint = 32;
static DATA_BYTES: uint = 32;
static ELEMENT_BITS: uint = 4;
static KEY_ELEMENTS: uint = KEY_BYTES * 8 / ELEMENT_BITS;
static NODE_CHILDREN: uint = 1 << ELEMENT_BITS;

static HEADER_SIZE: u64 = 1024;
static NODE_SIZE: u64 = 1024;

struct Node {
    // NodeData
    children: [u64, ..NODE_CHILDREN],
    child_hashes: [[u8, ..HASH_BYTES], ..NODE_CHILDREN],
    // leafData
    hash: [u8, ..HASH_BYTES],
    value: [u8, ..HASH_BYTES],
    // diskNode
    substring_length: u64,
    key_substring: [u8, ..KEY_BYTES],

}

impl Node {
    pub fn from_reader<T: Reader>(file: &mut T) -> IoResult<Node> {
        let mut node: Node = Default::default();
        for j in range(0, NODE_CHILDREN) {
            node.children[j] = try!(file.read_le_u64());
        }
        for j in range(0, NODE_CHILDREN) {
            copy_memory(node.child_hashes[j].as_mut_slice(), try!(file.read_exact(HASH_BYTES)).as_slice());
        }
        copy_memory(node.hash.as_mut_slice(), try!(file.read_exact(HASH_BYTES)).as_slice());
        copy_memory(node.value.as_mut_slice(), try!(file.read_exact(HASH_BYTES)).as_slice());
        node.substring_length = try!(file.read_le_u64());
        copy_memory(node.key_substring.as_mut_slice(), try!(file.read_exact(KEY_BYTES)).as_slice());
        return Ok(node);
    }
}

impl Default for Node {
    fn default() -> Node {
        Node {
            // NodeData
            children: [0, ..NODE_CHILDREN],
            child_hashes: [[0, ..HASH_BYTES], ..NODE_CHILDREN],
            // leafData
            hash: [0, ..HASH_BYTES],
            value: [0, ..HASH_BYTES],
            // diskNode
            substring_length: 0,
            key_substring: [0, ..KEY_BYTES],
        }
    }
}

pub struct MerkleMap {
    nodes: Vec<Node>,
}

impl MerkleMap {
    pub fn open<T: Reader+Seek>(file: &mut T) -> IoResult<MerkleMap> {
        let mut map = MerkleMap { nodes: Vec::new() };
        let items = try!(file.read_le_u64());

        for i in range(0, items) {
            try!(file.seek((HEADER_SIZE + i*NODE_SIZE) as i64, SeekSet));
            map.nodes.push(try!(Node::from_reader(file)));
        }

        return Ok(map);
    }

    pub fn nodes<'a>(&'a self) -> &'a Vec<Node> {
        &self.nodes
    }
}

