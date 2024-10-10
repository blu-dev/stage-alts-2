use self::types::{FilesystemInfo, ResServiceNX};

pub mod containers;
pub mod types;

#[skyline::from_offset(0x35406c0)]
pub unsafe fn increment_ref_count(table: &FilesystemInfo, index: u32);

#[skyline::from_offset(0x35407d0)]
pub unsafe fn decrement_ref_count(table: &FilesystemInfo, index: u32);

#[skyline::from_offset(0x3546270)]
pub unsafe fn add_to_resource_list(service: &ResServiceNX, index: u32, list_index: u32);
