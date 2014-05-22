#![crate_id = "merklemap#0.1"]
#![crate_type = "lib"]

use std::io::{IoResult, SeekSet};
use std::default::Default;
use std::slice::bytes::copy_memory;
use std::vec::Vec;

pub static KEY_BYTES: uint = 32;
pub static HASH_BYTES: uint = 32;
pub static DATA_BYTES: uint = 32;
pub static ELEMENT_BITS: uint = 4;
pub static KEY_ELEMENTS: uint = KEY_BYTES * 8 / ELEMENT_BITS;
pub static NODE_CHILDREN: uint = 1 << ELEMENT_BITS;

pub static HEADER_SIZE: u64 = 1024;
pub static NODE_SIZE: u64 = 1024;

#[deriving(Clone,Eq,TotalEq)]
pub struct Element(u8);

impl Element {
    pub fn from_bytes(bytes: &[u8]) -> Vec<Element> {
        let mut ret = Vec::with_capacity(bytes.len()*8/ELEMENT_BITS);
        
        assert_eq!(ELEMENT_BITS, 4);
        
        for b in bytes.iter() {
            ret.push(Element(b >> 4));
            ret.push(Element(b & 0x0F));
        }
        
        return ret;
    }

    pub fn to_bytes(elements: &[Element]) -> Vec<u8> {
        let mut ret = Vec::with_capacity((elements.len()*ELEMENT_BITS+7)/8);

        assert_eq!(ELEMENT_BITS, 4);
        for chunk in elements.chunks(2) {
            ret.push((chunk.get(0).unwrap().to_byte() << 4) | chunk.get(1).map_or(0, |&e| e.to_byte()));
        }

        return ret;
    }

    pub fn to_byte(&self) -> u8 {
        match self {
            &Element(e) => e
        }
    }
}

struct DiskNode {
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

impl DiskNode {
    pub fn from_reader<T: Reader>(file: &mut T) -> IoResult<DiskNode> {
        let mut node: DiskNode = Default::default();
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

impl Default for DiskNode {
    fn default() -> DiskNode {
        DiskNode {
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

pub struct Node {
    pub key: ~[Element],
    pub value: [u8, ..HASH_BYTES],
    pub children: [Option<Box<Node>>, ..NODE_CHILDREN],
    pub hash: [u8, ..HASH_BYTES],
}

pub enum TreePath {
    // Inner nodes that lead up to the target node
    Inode([u8, ..HASH_BYTES], ~[Element], [Option<Box<TreePath>>, ..NODE_CHILDREN]),
    // Inner node for which only its hash is known
    HashNode([u8, ..HASH_BYTES]),
    // Target node
    Onode([u8, ..HASH_BYTES], ~[Element]),
}

impl Node {
    fn find<'a>(&'a self, key: &[Element]) -> (Option<&'a [u8, ..HASH_BYTES]>, TreePath) {
        if key == self.key.as_slice() {
            (Some(&self.value), Onode(self.hash, key.to_owned()))
        } else {
            let mut value: Option<&'a [u8, ..HASH_BYTES]> = None;
            let mut children: [Option<Box<TreePath>>, ..NODE_CHILDREN] = unsafe { std::mem::init() };
            for (node, child) in children.mut_iter().zip(self.children.iter()) {
                *node = match child {
                    &Some(ref child) => Some(box HashNode(child.hash)),
                    &None => None,
                };
            }
            if key.starts_with(self.key.as_slice()) {
                match key.get(self.key.len()) {
                    Some(&Element(index)) => match self.children[index as uint] {
                        Some(ref node) => {
                            let (v, p) = node.find(key.slice_from(self.key.len()+1));
                            value = v;
                            children[index as uint] = Some(box p);
                        },
                        None => ()
                    },
                    None => ()
                }
            }
            (value, Inode(self.hash, self.key.as_slice().to_owned(), children))
        } 
    }
}

pub struct MerkleMap {
    pub root: Node,
}

impl MerkleMap {
    pub fn open<T: Reader+Seek>(file: &mut T, root_idx: uint) -> IoResult<MerkleMap> {
        let mut nodes = Vec::new();
        let items = try!(file.read_le_u64());

        nodes.push(Default::default());
        for i in range(0, items) {
            try!(file.seek((HEADER_SIZE + i*NODE_SIZE) as i64, SeekSet));
            nodes.push(try!(DiskNode::from_reader(file)));
        }

       return Ok(MerkleMap {
           root: MerkleMap::rebuild_node(if root_idx > 0 { root_idx } else { nodes.len()-1 }, &nodes)
       });
    }

    fn rebuild_node(idx: uint, nodes: &Vec<DiskNode>) -> Node {
        let mut ret = Node {
            key: {
                let mut key = Element::from_bytes(nodes.get(idx).key_substring);
                key.truncate(nodes.get(idx).substring_length as uint);
                key.as_slice().to_owned()
            },
            value: nodes.get(idx).value,
            // nullable pointer optimization
            children: unsafe { std::mem::init() },
            hash: nodes.get(idx).hash,
        };

        for (i, &child) in nodes.get(idx).children.iter().enumerate() {
            if child != 0 {
                ret.children[i] = Some(box MerkleMap::rebuild_node(child as uint, nodes));
            }
        }
        
        return ret;
    }
    
    pub fn lookup<'a>(&'a self, key: &[u8, ..KEY_BYTES]) -> (Option<&'a [u8, ..HASH_BYTES]>, TreePath) {
        self.root.find(Element::from_bytes(key.as_slice()).as_slice())
    }

    pub fn insert(&mut self, key: &[u8, ..KEY_BYTES], data: &[u8, ..HASH_BYTES]) {
        unimplemented!();
    }
}

impl Container for MerkleMap {
    fn len(&self) -> uint {
        unimplemented!();
    }
}

impl Map<[u8, ..KEY_BYTES], [u8, ..HASH_BYTES]> for MerkleMap {
    fn find<'a>(&'a self, key: &[u8, ..KEY_BYTES]) -> Option<&'a [u8, ..HASH_BYTES]> {
        let (value, _) = self.lookup(key);
        value
    }
}
