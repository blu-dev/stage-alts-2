use crate::{
    manager::MANAGER,
    search::SearchEx,
    utils::{ConcatHash, PrettyPath},
};
use smash_arc::*;

fn file_paths_mut(arc: &mut LoadedArc) -> &mut [FilePath] {
    unsafe {
        std::slice::from_raw_parts_mut(arc.file_paths as *mut FilePath, arc.get_file_paths().len())
    }
}

/// Attempts to patch a file path by replacing the "normal" or "battle"
/// with the same form but for the alt
fn patch_file_path(hash: Hash40, alt: usize) -> Option<PrettyPath> {
    let mut pretty = hash.pretty();
    (pretty.replace("normal", format!("normal_s{alt:02}").as_str())
        || pretty.replace("battle", format!("battle_s{alt:02}").as_str()))
    .then(|| pretty)
}

/// Patches the children of a dir info to use alt paths
pub fn patch_dir_info(arc: &mut LoadedArc, path: Hash40, alt: usize) {
    // If the dir info doesn't exist we can't patch it
    let Ok(dir_info) = arc.get_dir_info_from_hash(path).copied() else {
        log::error!("Failed to find dir info for {}", path.pretty());
        return;
    };

    for file_info in dir_info.file_info_range() {
        // Copy the info so we can see the indexes
        let info = arc.get_file_infos()[file_info];
        let path = arc.get_file_paths()[usize::from(info.file_path_index)]
            .path
            .hash40();

        // Try patching the path, if it fails then this isn't a stage form path
        let Some(alt_path) = patch_file_path(path, alt) else {
            log::warn!("Failed to find alt path for {}", path.pretty());
            continue;
        };

        // Get the file info from the alt path
        let Ok(alt_info) = arc.get_file_info_from_hash(alt_path.to_whole()).copied() else {
            log::error!("Failed to find file info for {}", alt_path);
            continue;
        };

        // Set the FileInfoIndex from the alt file info to the file info/file path
        file_paths_mut(arc)[usize::from(info.file_path_index)]
            .path
            .set_index(alt_info.file_info_indice_index.0);
        arc.get_file_infos_mut()[file_info].file_info_indice_index =
            alt_info.file_info_indice_index;
    }
}

/// Restores the children of a dir info to post-arcropolis filesystem state
pub fn restore_dir_info(arc: &mut LoadedArc, path: Hash40) {
    let mgr = MANAGER.read();

    // If the dir info doesn't exist we can't restore it
    let Ok(dir_info) = arc.get_dir_info_from_hash(path).copied() else {
        log::error!("Failed to find dir info for {}", path.pretty());
        return;
    };

    for file_info in dir_info.file_info_range() {
        // Get file info we are about to modify, copy it so we can have the indexes
        let info = arc.get_file_infos()[file_info];

        // Get the file path
        let path = arc.get_file_paths()[usize::from(info.file_path_index)]
            .path
            .hash40();

        // Look up the proper FileInfoIndex from the backup
        let Some(index) = mgr.backup_filepaths.get(&path) else {
            log::error!("Failed to find backup file path for {}", path.pretty());
            return;
        };

        // Set the FileInfoIndex in both the file info and file path
        file_paths_mut(arc)[usize::from(info.file_path_index)]
            .path
            .set_index(*index);
        arc.get_file_infos_mut()[file_info].file_info_indice_index = FileInfoIndiceIdx(*index);
    }
}

/// Patches a search section to use a certain alt, recursively
pub fn patch_search_section(search: &mut LoadedSearchSection, path: Hash40, alt: usize) {
    // If we can't get folder we can't patch
    let Ok(folder) = search.get_folder_path_entry_from_hash(path) else {
        log::error!("Failed to find search folder {}", path.pretty());
        return;
    };

    // Loop using child indexes
    let mut index = folder.get_first_child_index();
    while index < 0xFF_FFFF {
        // Backup prev for recursion check
        let prev = index;

        // Get path list entry
        let path = &search.get_path_list()[index];
        index = path.path.index() as usize;
        let path = path.path.hash40();

        // Attempt to patch the filepath, if we can't then this path shouldn't
        // even be in use
        let Some(alt_path) = patch_file_path(path, alt) else {
            log::error!("Failed to get alt path for {}", path.pretty());
            continue;
        };

        // Get the index of the alt path in the search section
        let Ok(alt_index) = search
            .get_path_index_from_hash(alt_path.to_whole())
            .copied()
        else {
            log::error!("Failed to get path index key from {}", alt_path,);
            continue;
        };

        // Get the mutable index of the base path in the search section
        let Ok(index) = search.get_path_index_from_hash_mut(path) else {
            log::error!("Failed to get path index key from {}", path.pretty());
            continue;
        };

        // Change the base path to point to the alt path
        index.set_index(alt_index.index());

        // Recursive
        if search.get_path_list()[prev].is_directory() {
            restore_search_section(search, path);
        }
    }
}

/// Restores a modified search section to post-arcropolis search, recursively
pub fn restore_search_section(search: &mut LoadedSearchSection, path: Hash40) {
    let mgr = MANAGER.read();

    // If we can't get the folder there's literally nothing we can do
    let Ok(folder) = search.get_folder_path_entry_from_hash(path) else {
        log::error!("Failed to find search folder {}", path.pretty());
        return;
    };

    // Loop using the child indexes
    let mut index = folder.get_first_child_index();
    while index < 0xFF_FFFF {
        // Keep the prev index because we are going to check it to perform
        // recursion later
        let prev = index;

        // Get the file path hash here
        let path = &search.get_path_list()[index];
        index = path.path.index() as usize;
        let path = path.path.hash40();

        // If we can't get the index to modify from the live search section
        // then we can't restore it
        let Ok(index) = search.get_path_index_from_hash_mut(path) else {
            log::error!("Failed to get path index key from {}", path.pretty());
            continue;
        };

        // If it's not in the backup,
        // 1.) Something really fucky is going on
        // 2.) We can't restore
        let Some(base_index) = mgr.backup_searchpaths.get(&path) else {
            log::error!("Failed to get backup path index key from {}", path.pretty());
            continue;
        };

        index.set_index(*base_index);

        // Check recursively
        if search.get_path_list()[prev].is_directory() {
            restore_search_section(search, path);
        }
    }
}
