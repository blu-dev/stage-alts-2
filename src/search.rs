use smash_arc::{
    ArcLookup, FolderPathListEntry, Hash40, HashToIndex, LoadedArc, LoadedSearchSection,
    LookupError, PathListEntry, SearchLookup,
};
use std::{
    collections::{BTreeMap, HashMap},
    fmt::Display,
};

use crate::{
    manager::{AltInfo, StageInfo, StageKind, UiPaths},
    resources::types::FilesystemInfo,
    utils::ConcatHash,
};

pub trait SearchEx: SearchLookup {
    fn get_folder_path_to_index_mut(&mut self) -> &mut [HashToIndex];
    fn get_folder_path_list_mut(&mut self) -> &mut [FolderPathListEntry];
    fn get_path_to_index_mut(&mut self) -> &mut [HashToIndex];
    fn get_path_list_indices_mut(&mut self) -> &mut [u32];
    fn get_path_list_mut(&mut self) -> &mut [PathListEntry];

    fn get_folder_path_index_from_hash_mut(
        &mut self,
        hash: impl Into<Hash40>,
    ) -> Result<&mut HashToIndex, LookupError> {
        let folder_path_to_index = self.get_folder_path_to_index_mut();
        match folder_path_to_index.binary_search_by_key(&hash.into(), |h| h.hash40()) {
            Ok(idx) => Ok(&mut folder_path_to_index[idx]),
            Err(_) => Err(LookupError::Missing),
        }
    }

    fn get_folder_path_entry_from_hash_mut(
        &mut self,
        hash: impl Into<Hash40>,
    ) -> Result<&mut FolderPathListEntry, LookupError> {
        let index = *self.get_folder_path_index_from_hash(hash)?;
        if index.index() != 0xFF_FFFF {
            Ok(&mut self.get_folder_path_list_mut()[index.index() as usize])
        } else {
            Err(LookupError::Missing)
        }
    }

    fn get_path_index_from_hash_mut(
        &mut self,
        hash: impl Into<Hash40>,
    ) -> Result<&mut HashToIndex, LookupError> {
        let path_to_index = self.get_path_to_index_mut();
        match path_to_index.binary_search_by_key(&hash.into(), |h| h.hash40()) {
            Ok(idx) => Ok(&mut path_to_index[idx]),
            Err(_) => Err(LookupError::Missing),
        }
    }

    fn get_path_list_index_from_hash_mut(
        &mut self,
        hash: impl Into<Hash40>,
    ) -> Result<&mut u32, LookupError> {
        let index = *self.get_path_index_from_hash(hash)?;
        if index.index() != 0xFF_FFFF {
            Ok(&mut self.get_path_list_indices_mut()[index.index() as usize])
        } else {
            Err(LookupError::Missing)
        }
    }

    fn get_path_list_entry_from_hash_mut(
        &mut self,
        hash: impl Into<Hash40>,
    ) -> Result<&mut PathListEntry, LookupError> {
        let index = self.get_path_list_index_from_hash(hash)?;
        if index != 0xFF_FFFF {
            Ok(&mut self.get_path_list_mut()[index as usize])
        } else {
            Err(LookupError::Missing)
        }
    }

    fn get_first_child_in_folder_mut(
        &mut self,
        hash: impl Into<Hash40>,
    ) -> Result<&mut PathListEntry, LookupError> {
        let folder_path = self.get_folder_path_entry_from_hash(hash)?;
        let index_idx = folder_path.get_first_child_index();

        if index_idx == 0xFF_FFFF {
            return Err(LookupError::Missing);
        }

        let path_entry_index = self.get_path_list_indices()[index_idx];
        if path_entry_index != 0xFF_FFFF {
            Ok(&mut self.get_path_list_mut()[path_entry_index as usize])
        } else {
            Err(LookupError::Missing)
        }
    }

    fn get_next_child_in_folder_mut(
        &mut self,
        current_child: &PathListEntry,
    ) -> Result<&mut PathListEntry, LookupError> {
        let index_idx = current_child.path.index() as usize;
        if index_idx == 0xFF_FFFF {
            return Err(LookupError::Missing);
        }

        let path_entry_index = self.get_path_list_indices()[index_idx];
        if path_entry_index != 0xFF_FFFF {
            Ok(&mut self.get_path_list_mut()[path_entry_index as usize])
        } else {
            Err(LookupError::Missing)
        }
    }
}

impl SearchEx for LoadedSearchSection {
    fn get_folder_path_to_index_mut(&mut self) -> &mut [HashToIndex] {
        unsafe {
            let table_size = (*self.body).folder_path_count;
            std::slice::from_raw_parts_mut(self.folder_path_index as _, table_size as usize)
        }
    }

    fn get_folder_path_list_mut(&mut self) -> &mut [FolderPathListEntry] {
        unsafe {
            let table_size = (*self.body).folder_path_count;
            std::slice::from_raw_parts_mut(self.folder_path_list as _, table_size as usize)
        }
    }

    fn get_path_to_index_mut(&mut self) -> &mut [HashToIndex] {
        unsafe {
            let table_size = (*self.body).path_indices_count;
            std::slice::from_raw_parts_mut(self.path_index as _, table_size as usize)
        }
    }

    fn get_path_list_indices_mut(&mut self) -> &mut [u32] {
        unsafe {
            let table_size = (*self.body).path_indices_count;
            std::slice::from_raw_parts_mut(self.path_list_indices as _, table_size as usize)
        }
    }

    fn get_path_list_mut(&mut self) -> &mut [PathListEntry] {
        unsafe {
            let table_size = (*self.body).path_count;
            std::slice::from_raw_parts_mut(self.path_list as _, table_size as usize)
        }
    }
}

enum SearchKey<'a> {
    Resolved(&'a str),
    Unresolved(Hash40),
}

impl<'a> SearchKey<'a> {
    pub fn new(lookup: &'a HashMap<Hash40, String>, hash: Hash40) -> Self {
        match lookup.get(&hash) {
            Some(unhashed) => Self::Resolved(unhashed.as_str()),
            None => Self::Unresolved(hash),
        }
    }
}

impl Display for SearchKey<'_> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Resolved(value) => f.write_str(value),
            Self::Unresolved(value) => write!(f, "{:#x}", value.0),
        }
    }
}

impl PartialEq for SearchKey<'_> {
    fn eq(&self, other: &Self) -> bool {
        self.cmp(other) == std::cmp::Ordering::Equal
    }
}

impl Eq for SearchKey<'_> {}

impl PartialOrd for SearchKey<'_> {
    fn partial_cmp(&self, other: &Self) -> Option<std::cmp::Ordering> {
        Some(self.cmp(other))
    }
}

impl Ord for SearchKey<'_> {
    fn cmp(&self, other: &Self) -> std::cmp::Ordering {
        match (self, other) {
            (Self::Resolved(first), Self::Resolved(second)) => first.cmp(second),
            (Self::Resolved(_), Self::Unresolved(_)) => std::cmp::Ordering::Less,
            (Self::Unresolved(_), Self::Resolved(_)) => std::cmp::Ordering::Greater,
            (Self::Unresolved(first), Self::Unresolved(second)) => first.0.cmp(&second.0),
        }
    }
}

struct SearchEntry<'a> {
    key: SearchKey<'a>,
    is_folder: bool,
}

impl PartialEq for SearchEntry<'_> {
    fn eq(&self, other: &Self) -> bool {
        self.cmp(other) == std::cmp::Ordering::Equal
    }
}

impl Eq for SearchEntry<'_> {}

impl PartialOrd for SearchEntry<'_> {
    fn partial_cmp(&self, other: &Self) -> Option<std::cmp::Ordering> {
        Some(self.cmp(other))
    }
}

impl Ord for SearchEntry<'_> {
    fn cmp(&self, other: &Self) -> std::cmp::Ordering {
        if self.is_folder == other.is_folder {
            self.key.cmp(&other.key)
        } else {
            (!self.is_folder as usize).cmp(&(!other.is_folder as usize))
        }
    }
}

trait IndexSettable {
    fn set_index(&mut self, index: Option<usize>);
}

impl IndexSettable for FolderPathListEntry {
    fn set_index(&mut self, index: Option<usize>) {
        self.set_first_child_index(index.unwrap_or(0x00FF_FFFF) as u32);
    }
}

impl IndexSettable for PathListEntry {
    fn set_index(&mut self, index: Option<usize>) {
        self.path.set_index(index.unwrap_or(0x00FF_FFFF) as u32);
    }
}

/// Sorts the folder contents (recursively) of a folder such that known hashes (all vanilla file hashes)
/// are ordered before new files.
pub fn sort_folder_contents(
    name: Hash40,
    search: &mut LoadedSearchSection,
    lookup: &HashMap<Hash40, String>,
) {
    let Ok(folder) = search.get_folder_path_entry_from_hash(name) else {
        log::warn!("Failed to get folder '{}", SearchKey::new(lookup, name));
        return;
    };

    let mut children = BTreeMap::new();

    let mut index = folder.get_first_child_index();

    while index < 0x00FF_FFFF {
        let child = &search.get_path_list()[index];
        children.insert(
            SearchEntry {
                key: SearchKey::new(lookup, child.file_name.hash40()),
                is_folder: child.is_directory(),
            },
            index,
        );

        index = child.path.index() as usize;

        if child.is_directory() {
            sort_folder_contents(child.path.hash40(), search, lookup);
        }
    }

    let Ok(folder) = search.get_folder_path_entry_from_hash_mut(name) else {
        log::warn!(
            "Failed to get first child in folder '{}",
            SearchKey::new(lookup, name)
        );
        return;
    };

    let mut current: &mut dyn IndexSettable = folder;

    for index in children.into_values() {
        current.set_index(Some(index));
        current = &mut search.get_path_list_mut()[index];
    }

    current.set_index(None);
}

pub fn get_search_lookup() -> HashMap<Hash40, String> {
    let hash_data = std::fs::read_to_string("sd:/ultimate/stage-alts/Hashes_all").unwrap();

    HashMap::from_iter(
        hash_data
            .lines()
            .map(|line| (Hash40::from(line.trim()), line.trim().to_string())),
    )
}

fn guess_hash(hash: Hash40) -> Option<(usize, bool)> {
    match hash.len() {
        10 => {
            for x in 1..100 {
                let mut string = format!("normal_s{x:02}");
                if Hash40::from(string.as_str()) == hash {
                    return Some((x, true));
                }

                string.replace_range(..6, "battle");
                if Hash40::from(string.as_str()) == hash {
                    return Some((x, false));
                }
            }
        }
        11 => {
            for x in 100..1000 {
                let mut string = format!("normal_s{x:03}");
                if Hash40::from(string.as_str()) == hash {
                    return Some((x, true));
                }

                string.replace_range(..6, "battle");
                if Hash40::from(string.as_str()) == hash {
                    return Some((x, false));
                }
            }
        }
        _ => {}
    }

    None
}

pub fn build_alt_lookups() -> BTreeMap<StageInfo, Vec<AltInfo>> {
    let search = FilesystemInfo::instance().unwrap().search();

    let Ok(folder) = search.get_folder_path_entry_from_hash("stage") else {
        log::error!("Can't find stage folder -- what the FUCK did you do?");
        return Default::default();
    };

    let mut index = folder.get_first_child_index();

    let mut map: BTreeMap<StageInfo, Vec<_>> = BTreeMap::new();

    while index < 0x00FF_FFFF {
        let parent = search.get_path_list()[index];
        index = parent.path.index() as usize;

        if !parent.is_directory() {
            continue;
        }

        let Ok(child_folder) = search.get_folder_path_entry_from_hash(parent.path.hash40()) else {
            log::error!(
                "Failed to get folder entry for {}",
                parent.path.hash40().pretty()
            );
            continue;
        };

        let mut child_index = child_folder.get_first_child_index();

        while child_index < 0x00FF_FFFF {
            let path = search.get_path_list()[child_index];
            child_index = path.path.index() as usize;

            if let Some((alt_id, is_normal)) = guess_hash(path.file_name.hash40()) {
                map.entry(StageInfo {
                    name: child_folder.file_name.hash40(),
                    normal_form: is_normal,
                })
                .or_default()
                .push(AltInfo {
                    slot_value: alt_id,
                    wifi_safe: true,
                    ui_paths: UiPaths::new(StageKind::from(parent.file_name.hash40()), alt_id),
                });
            }
        }
    }

    map.values_mut()
        .for_each(|value| value.sort_by_key(|info| info.slot_value));

    map
}

pub fn is_descendant_of(path: Hash40, ancestor: Hash40) -> bool {
    let search = FilesystemInfo::instance().unwrap().search();

    let Ok(mut path) = search.get_path_list_entry_from_hash(path) else {
        return false;
    };

    loop {
        if path.parent.hash40() == ancestor {
            return true;
        }

        let Ok(new_path) = search.get_path_list_entry_from_hash(path.parent.hash40()) else {
            return false;
        };

        path = new_path;
    }
}

pub fn recreate_search_path_hash<'a>(
    search: &'a LoadedSearchSection,
    mut path: &'a PathListEntry,
    alt: usize,
) -> Option<Hash40> {
    let mut components = vec![];

    while path.file_name.hash40() != Hash40::from("normal")
        && path.file_name.hash40() != Hash40::from("battle")
    {
        components.push(path.file_name.hash40());
        let Ok(parent) = search.get_path_list_entry_from_hash(path.parent.hash40()) else {
            log::error!(
                "Failed to get parent path entry, parent is '{}'",
                path.parent.hash40().pretty()
            );
            return None;
        };

        path = parent;
    }

    let new_name = if alt == 0 {
        path.file_name.hash40()
    } else {
        path.file_name
            .hash40()
            .concat(format!("_s{alt:02}").as_str())
    };

    let mut full_path = path.parent.hash40().concat("/").concat(new_name);
    for component in components.into_iter().rev() {
        full_path = full_path.concat("/").concat(component);
    }

    Some(full_path)
}

pub fn collect_files_from_path(
    arc: &LoadedArc,
    search: &LoadedSearchSection,
    path: Hash40,
    alt: usize,
    needs_alt_fix: bool,
) -> Vec<u32> {
    let folder = if needs_alt_fix {
        let Ok(search_path) = search.get_path_list_entry_from_hash(path) else {
            log::error!("Unable to get search path '{}'", path.pretty());
            return vec![];
        };

        let Some(alt_path) = recreate_search_path_hash(search, search_path, alt) else {
            log::error!("Unable to get alt path for '{}'", path.pretty());
            return vec![];
        };

        let Ok(folder) = search.get_folder_path_entry_from_hash(alt_path) else {
            log::info!("Did not get search path entry for '{}'", path.pretty());
            return vec![];
        };

        folder
    } else {
        let Ok(folder) = search.get_folder_path_entry_from_hash(path) else {
            log::info!("Did not get search path entry for '{}'", path.pretty());
            return vec![];
        };

        folder
    };

    let mut index = folder.get_first_child_index();
    let mut files = vec![];

    while index < 0x00FF_FFFF {
        let path = &search.get_path_list()[index];
        index = path.path.index() as usize;

        if path.is_directory() {
            files.extend(collect_files_from_path(
                arc,
                search,
                path.path.hash40(),
                alt,
                false,
            ));
            continue;
        }

        let Ok(index) = arc.get_file_path_index_from_hash(path.path.hash40()) else {
            log::error!(
                "Failed to get file path index for file '{}' while collecting '{}'",
                path.path.hash40().pretty(),
                folder.path.hash40().pretty(),
            );
            continue;
        };

        files.push(index.0);
    }

    files
}
