use std::{collections::HashMap, fmt::Display};

use locks::Mutex;
use smash_arc::{Hash40, SearchLookup};

use crate::resources::types::FilesystemInfo;

static HASH_LOOKUP: Mutex<Option<&'static HashMap<Hash40, String>>> = Mutex::new(None);

pub struct PrettyPath {
    lookup: &'static HashMap<Hash40, String>,
    components: Vec<Hash40>,
}

impl PrettyPath {
    pub fn sub_range(&self, len: usize) -> Hash40 {
        let mut hash = Hash40(0);
        for component in self.components.iter().take(len).copied() {
            if hash.0 != 0 {
                hash = hash.concat("/");
            }
            hash = hash.concat(component);
        }

        hash
    }

    pub fn components(&self) -> &[Hash40] {
        &self.components
    }

    pub fn replace(&mut self, search: impl Into<Hash40>, replace: impl Into<Hash40>) -> bool {
        let search = search.into();
        let replace = replace.into();

        let mut replaced = false;
        for component in self.components.iter_mut() {
            if *component == search {
                *component = replace;
                replaced = true;
            }
        }

        replaced
    }

    pub fn to_whole(&self) -> Hash40 {
        let mut hash = Hash40(0);
        for component in self.components.iter().copied() {
            if hash.0 != 0 {
                hash = hash.concat("/");
            }
            hash = hash.concat(component);
        }

        hash
    }
}

impl Display for PrettyPath {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        for component in self.components.iter() {
            f.write_str("/")?;
            if let Some(pretty) = self.lookup.get(component) {
                f.write_str(pretty.as_str())?;
            } else {
                write!(f, "{:#010x}", component.0)?;
            }
        }

        Ok(())
    }
}

pub trait ConcatHash {
    fn concat(self, extra: impl Into<Hash40>) -> Self;
    fn pretty(self) -> PrettyPath;
}

impl ConcatHash for Hash40 {
    fn concat(self, extra: impl Into<Hash40>) -> Self {
        let raw = self.0;
        let extra_raw = extra.into().0;

        let new = hash40::Hash40(raw).concat(hash40::Hash40(extra_raw)).0;
        Hash40(new)
    }

    fn pretty(self) -> PrettyPath {
        if HASH_LOOKUP.lock().is_none() {
            init_hash_lookup(true);
        }

        let lookup = HASH_LOOKUP.lock().unwrap();

        let Some(search) = FilesystemInfo::instance().map(|fs| fs.search()) else {
            return PrettyPath {
                lookup,
                components: vec![self],
            };
        };

        let mut components = vec![];

        let mut path = self;
        while let Ok(entry) = search.get_path_list_entry_from_hash(path) {
            components.push(entry.file_name.hash40());
            path = entry.parent.hash40();

            if path == Hash40::from("/") {
                break;
            }
        }

        components.reverse();

        PrettyPath { lookup, components }
    }
}

pub fn init_hash_lookup(empty: bool) {
    if empty {
        *HASH_LOOKUP.lock() = Some(Box::leak(Box::new(HashMap::new())));
    } else {
        *HASH_LOOKUP.lock() = Some(Box::leak(Box::new(crate::search::get_search_lookup())));
    }
}
