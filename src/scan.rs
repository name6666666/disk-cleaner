use rayon::prelude::*;
use std::fs;
use std::path::Path;
use serde_json::{Value, json};


enum Item {
    Dir {
        name: String,
        size: u64,
        children: Vec<Item>,
    },
    File {
        name: String,
        size: u64,
    },
}

impl Item {
    fn get_size(&self) -> &u64 {
        match self {
            Item::Dir { size, .. } => size,
            Item::File { size, .. } => size,
        }
    }
    fn to_json(&self, min_size: u64) -> Value {
        match self {
            Item::Dir { name, size, children } => json!({
                "type": "dir",
                "name": name,
                "size_mb": convert_size(*size),
                "children": if *size >= min_size {
                    json!(children.iter().map(|item| item.to_json(min_size)).collect::<Vec<_>>())
                } else {
                    json!("ellipsized")
                },
            }),
            Item::File { name, size } => json!({
                "type": "file",
                "name": name,
                "size_mb": convert_size(*size),
            }),
        }
    }
}

pub struct ScanResult {
    items: Vec<Item>,
    size: u64,
}

pub const MB: u64 = 1024 * 1024;
pub fn convert_size(size: u64) -> f64 {
    const REMAIN: f64 = 1000.0;
    let result = size as f64 / MB as f64;
    (result * REMAIN).trunc() / REMAIN
}
impl ScanResult {
    pub fn optimised_folding(&self, target_byte: usize) -> String {
        let min_target_byte = (target_byte as f64 * 0.8) as usize;
        let mut boarder = (0u64, 20000 * MB);
        let mut ret = String::new();
        for _ in 0..20 {
            let min_size = boarder.0 + (boarder.1 - boarder.0) / 2;
            ret = self.to_json(min_size).to_string();
            let len = ret.len();
            if len < min_target_byte {
                boarder.1 = min_size;
            } else if len > target_byte {
                boarder.0 = min_size;
            } else {
                break;
            }
        }
        ret
    }
    pub fn get_size(&self) -> f64 {
        convert_size(self.size)
    }
    pub fn new<P: AsRef<Path>>(dir: P) -> Result<Self, ()> {
        let (items, size) = Self::walk_dir(dir.as_ref())?;
        Ok(Self {
            items,
            size,
        })
    }
    fn walk_dir(dir: &Path) -> Result<(Vec<Item>, u64), ()> {
        let mut items: Vec<Item> = {
            match fs::read_dir(dir) {
                Ok(entries) => entries,
                Err(_) => return Err(()),
            }
        }.into_iter().par_bridge().filter_map(|entry| {
            let entry = entry.unwrap();
            if entry.file_type().unwrap().is_dir() {
                match Self::walk_dir(entry.path().as_path()) {
                    Ok((items, size)) => Some(
                        Item::Dir {
                            name: entry.file_name().into_string().unwrap(),
                            size,
                            children: items,
                        }
                    ),
                    Err(_) => None,
                }
            } else if entry.file_type().unwrap().is_file() {
                Some(Item::File {
                    name: entry.file_name().into_string().unwrap(),
                    size: entry.metadata().unwrap().len(),
                })
            } else {
                None
            }
        }).collect();
        items.sort_by(|a, b| {
            a.get_size().cmp(b.get_size())
        });
        let size: u64 = items.iter().map(|x| x.get_size()).sum();
        Ok((items, size))
    }
    fn to_json(&self, min_size: u64) -> Value {
        json!({
            "items": self.items.iter().map(|x| x.to_json(min_size)).collect::<Vec<Value>>(),
            "size_mb": convert_size(self.size),
        })
    }
}
