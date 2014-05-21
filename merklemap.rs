extern crate merklemap;
extern crate serialize;

use merklemap::MerkleMap;
use std::os::args;
use std::io::File;
use serialize::hex::ToHex;
use std::from_str::FromStr;

fn print_values(tree: &merklemap::Node, prefix: &str) {
    let mut new_prefix = StrBuf::with_capacity(prefix.len()+tree.key.len()+1);
    new_prefix.push_str(prefix);
    for e in tree.key.iter() {
        new_prefix.push_char(std::char::from_digit(e.to_byte() as uint, 16).unwrap());
    }

    if new_prefix.len() == merklemap::KEY_ELEMENTS {
        println!("{} => {}", new_prefix, tree.value.to_hex());
    }
    
    for (i, child) in tree.children.iter().enumerate() {
        match child {
            &Some(ref child) => {
                new_prefix.push_char(std::char::from_digit(i, 16).unwrap());
                print_values(&**child, new_prefix.as_slice());
                new_prefix.pop_char();
            }
            &None => ()
        }
    }
}

fn main() {
    let argv = args();
    let mut file = File::open(&Path::new(argv.get(1).as_slice())).unwrap();
    let tree = if argv.len() == 3 {
         MerkleMap::open(&mut file, FromStr::from_str(argv.get(2).as_slice()).unwrap()).unwrap()
    } else {
         MerkleMap::open(&mut file, 0).unwrap()
    };
    print_values(&tree.root, "");
}
