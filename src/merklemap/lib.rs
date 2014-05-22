#![crate_id = "merklemap#0.1"]
#![crate_type = "lib"]

extern crate crypto = "rust-crypto";

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
                let Element(index) = key[self.key.len()];
                match self.children[index as uint] {
                    Some(ref node) => {
                        let (v, p) = node.find(key.slice_from(self.key.len()+1));
                        value = v;
                        children[index as uint] = Some(box p);
                    },
                    None => ()
                };
            }
            (value, Inode(self.hash, self.key.as_slice().to_owned(), children))
        } 
    }

    fn rehash(&mut self) {
        unimplemented!();
    }

    fn swap(&mut self, k: &[Element], v: [u8, ..HASH_BYTES]) -> Option<[u8, ..HASH_BYTES]> {
        if k == self.key.as_slice() {
            let mut v = v;
            std::mem::swap(&mut self.value, &mut v);
            Some(v)
        } else if k.starts_with(self.key.as_slice()) {
            let Element(index) = k[self.key.len()];
            let ret = match self.children[index as uint] {
                Some(ref mut node) => node.swap(k.slice_from(self.key.len()+1), v),
                ref mut node => {
                    let mut child = Some(box Node {
                        key: k.slice_from(self.key.len()+1).to_owned(),
                        value: v,
                        children: unsafe { std::mem::init() },
                        hash: unimplemented!(),
                    });
                    std::mem::swap(node, &mut child);
                    None
                }
            };
            self.rehash();
            ret
        } else {
            unimplemented!() // find common prefix and create a parent node
        }
    }

    fn pop(&mut self, k: &[Element]) -> Option<[u8, ..HASH_BYTES]> {
        if k == self.key.as_slice() {
            self.key = box [];
            let mut ret = [0, ..HASH_BYTES];
            std::mem::swap(&mut self.value, &mut ret);
            Some(ret)
        } else if k.starts_with(self.key.as_slice()) {
            let Element(index) = k[self.key.len()];
            let ret = match self.children[index as uint] {
                ref mut node if k.len() + 1 == self.key.len() => {
                    let mut ret = None;
                    std::mem::swap(node, &mut ret);
                    ret.map(|node| node.value)
                },
                Some(ref mut node) => node.pop(k.slice_from(self.key.len()+1)),
                None => None
            };
            for child in self.children.mut_iter() {
                match child {
                    &Some(ref c) if self.key.len()+c.key.len()+1 != KEY_ELEMENTS && c.children.iter().count(|c| c.is_some()) == 0 => (),
                    _ => continue
                }
                *child = None;
            }
            if self.children.iter().count(|c| c.is_some()) == 1 {
                let c = {
                    let (c, node) = self.children.iter().enumerate().find(|&(_, c)| c.is_some()).unwrap();
                    let node = node.as_ref().unwrap();
                    self.key = {
                        let mut key = Vec::with_capacity(self.key.len()+1+node.key.len());
                        key.push_all(self.key);
                        key.push(Element(c as u8));
                        key.push_all(node.key);
                        key.as_slice().to_owned()
                    };
                    self.value = node.value;
                    c
                };
                self.children[c] = None;
            }
            self.rehash();
            ret
        } else {
            None
        }
    }
}

pub struct MerkleMap {
    pub root: Node,
    length: uint,
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
        
        let (root, items) = MerkleMap::rebuild_node(if root_idx > 0 { root_idx } else { nodes.len()-1 }, &nodes, 0);
        Ok(MerkleMap {
            root: root,
            length: items
        })
    }

    fn rebuild_node(idx: uint, nodes: &Vec<DiskNode>, prefix_len: uint) -> (Node, uint) {
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

        let mut items = if prefix_len+ret.key.len() == KEY_ELEMENTS { 1 } else { 0 };
        
        for (i, &child) in nodes.get(idx).children.iter().enumerate() {
            if child != 0 {
                let (tree, tree_items) = MerkleMap::rebuild_node(child as uint, nodes, prefix_len+ret.key.len()+1);
                ret.children[i] = Some(box tree);
                items += tree_items;
            }
        }
        
        return (ret, items);
    }
    
    pub fn lookup<'a>(&'a self, key: &[u8, ..KEY_BYTES]) -> (Option<&'a [u8, ..HASH_BYTES]>, TreePath) {
        self.root.find(Element::from_bytes(key.as_slice()).as_slice())
    }
}

impl Container for MerkleMap {
    fn len(&self) -> uint {
        self.length
    }
}

impl Map<[u8, ..KEY_BYTES], [u8, ..HASH_BYTES]> for MerkleMap {
    fn find<'a>(&'a self, key: &[u8, ..KEY_BYTES]) -> Option<&'a [u8, ..HASH_BYTES]> {
        let (value, _) = self.lookup(key);
        value
    }
}

impl Mutable for MerkleMap {
    fn clear(&mut self) {
        self.root = Node {
            key: box [],
            value: [0, ..HASH_BYTES],
            children: unsafe { std::mem::init() },
            hash: [0, ..HASH_BYTES]
        };
        self.length = 0;
    }
}

impl MutableMap<[u8, ..KEY_BYTES], [u8, ..HASH_BYTES]> for MerkleMap {
    fn swap(&mut self, k: [u8, ..KEY_BYTES], v: [u8, ..HASH_BYTES]) -> Option<[u8, ..HASH_BYTES]> {
        self.root.swap(Element::from_bytes(k).as_slice(), v)
    }

    fn pop(&mut self, k: &[u8, ..KEY_BYTES]) -> Option<[u8, ..HASH_BYTES]> {
        self.root.pop(Element::from_bytes(k.as_slice()).as_slice())
    }
    
    fn find_mut<'a>(&'a mut self, _key: &[u8, ..KEY_BYTES]) -> Option<&'a mut [u8, ..HASH_BYTES]> {
        unimplemented!();
    }
}
