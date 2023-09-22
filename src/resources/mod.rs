use self::types::{FilesystemInfo, ResServiceNX};

pub mod containers;
pub mod types;

#[skyline::from_offset(0x353fa20)]
pub unsafe fn increment_ref_count(table: &FilesystemInfo, index: u32);

#[skyline::from_offset(0x353fb30)]
pub unsafe fn decrement_ref_count(table: &FilesystemInfo, index: u32);

#[skyline::from_offset(0x35455d0)]
pub unsafe fn add_to_resource_list(service: &ResServiceNX, index: u32, list_index: u32);
