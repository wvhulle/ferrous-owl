use std::collections::{HashMap, HashSet};

use rayon::prelude::*;
use rustc_borrowck::consumers::{PoloniusLocationTable, PoloniusOutput};
use rustc_index::Idx;
use rustc_middle::mir::Local;

use crate::{
    mir_transform::{BorrowData, BorrowMap, rich_locations_to_ranges},
    models::{MirBasicBlock, Range},
    range_ops,
};

#[must_use]
pub fn get_accurate_live(
    datafrog: &PoloniusOutput,
    location_table: &PoloniusLocationTable,
    basic_blocks: &[MirBasicBlock],
) -> HashMap<Local, Vec<Range>> {
    get_range(
        datafrog
            .var_live_on_entry
            .iter()
            .map(|(p, v)| (*p, v.iter().copied())),
        location_table,
        basic_blocks,
    )
}

/// returns (shared, mutable)
pub fn get_borrow_live(
    datafrog: &PoloniusOutput,
    location_table: &PoloniusLocationTable,
    borrow_map: &BorrowMap,
    basic_blocks: &[MirBasicBlock],
) -> (HashMap<Local, Vec<Range>>, HashMap<Local, Vec<Range>>) {
    let output = datafrog;
    let mut shared_borrows = HashMap::new();
    let mut mutable_borrows = HashMap::new();
    for (location_idx, borrow_idc) in &output.loan_live_at {
        let location = location_table.to_rich_location(*location_idx);
        for borrow_idx in borrow_idc {
            match borrow_map.get_from_borrow_index(*borrow_idx) {
                Some((_, BorrowData::Shared { borrowed, .. })) => {
                    shared_borrows
                        .entry(*borrowed)
                        .or_insert_with(Vec::new)
                        .push(location);
                }
                Some((_, BorrowData::Mutable { borrowed, .. })) => {
                    mutable_borrows
                        .entry(*borrowed)
                        .or_insert_with(Vec::new)
                        .push(location);
                }
                _ => {}
            }
        }
    }
    (
        shared_borrows
            .into_par_iter()
            .map(|(local, locations)| {
                (
                    local,
                    range_ops::eliminated_ranges(rich_locations_to_ranges(
                        basic_blocks,
                        &locations,
                    )),
                )
            })
            .collect(),
        mutable_borrows
            .into_par_iter()
            .map(|(local, locations)| {
                (
                    local,
                    range_ops::eliminated_ranges(rich_locations_to_ranges(
                        basic_blocks,
                        &locations,
                    )),
                )
            })
            .collect(),
    )
}

pub fn get_must_live(
    datafrog: &PoloniusOutput,
    location_table: &PoloniusLocationTable,
    borrow_map: &BorrowMap,
    basic_blocks: &[MirBasicBlock],
) -> HashMap<Local, Vec<Range>> {
    // obtain a map that region -> region contained locations
    let mut region_locations = HashMap::new();
    for (location_idx, region_idc) in &datafrog.origin_live_on_entry {
        for region_idx in region_idc {
            region_locations
                .entry(*region_idx)
                .or_insert_with(HashSet::new)
                .insert(*location_idx);
        }
    }

    // obtain maps from borrow index to locals:
    // - borrow_assigned_local: borrow index -> local that holds the reference
    // - borrow_borrowed_local: borrow index -> local being borrowed
    let mut borrow_assigned_local = HashMap::new();
    let mut borrow_borrowed_local = HashMap::new();
    for (local, borrow_idc) in borrow_map.local_map() {
        for borrow_idx in borrow_idc {
            borrow_assigned_local.insert(*borrow_idx, *local);
        }
    }
    // Get the borrowed local from BorrowData
    for (borrow_idx, (_location, borrow_data)) in borrow_map.iter_with_index() {
        let borrowed = match borrow_data {
            BorrowData::Shared { borrowed, .. } | BorrowData::Mutable { borrowed, .. } => borrowed,
        };
        borrow_borrowed_local.insert(borrow_idx, *borrowed);
    }

    // check all regions' subset that must be satisfied
    let mut subsets = HashMap::new();
    for subset in datafrog.subset.values() {
        for (sup, subs) in subset {
            subsets
                .entry(*sup)
                .or_insert_with(HashSet::new)
                .extend(subs.iter().copied());
        }
    }
    // obtain a map that region -> locations
    // a region must contains the locations
    let mut region_must_locations = HashMap::new();
    for (sup, subs) in &subsets {
        for sub in subs {
            if let Some(locs) = region_locations.get(sub) {
                region_must_locations
                    .entry(*sup)
                    .or_insert_with(HashSet::new)
                    .extend(locs.iter().copied());
            }
        }
    }
    // obtain a map that local -> locations
    // a local must lives in the locations
    // This includes BOTH the local holding the reference AND the local being
    // borrowed
    let mut local_must_locations = HashMap::new();
    for region_borrows in datafrog.origin_contains_loan_at.values() {
        for (region, borrows) in region_borrows {
            for borrow in borrows {
                if let Some(locs) = region_must_locations.get(region) {
                    // Track must-live for the local that holds the reference
                    if let Some(assigned_local) = borrow_assigned_local.get(borrow) {
                        local_must_locations
                            .entry(*assigned_local)
                            .or_insert_with(HashSet::new)
                            .extend(locs.iter().copied());
                    }
                    // Also track must-live for the borrowed local (the source of the borrow)
                    if let Some(borrowed_local) = borrow_borrowed_local.get(borrow) {
                        local_must_locations
                            .entry(*borrowed_local)
                            .or_insert_with(HashSet::new)
                            .extend(locs.iter().copied());
                    }
                }
            }
        }
    }

    local_must_locations
        .iter()
        .map(|(local, locations)| {
            (
                *local,
                range_ops::eliminated_ranges(rich_locations_to_ranges(
                    basic_blocks,
                    &locations
                        .iter()
                        .map(|v| location_table.to_rich_location(*v))
                        .collect::<Vec<_>>(),
                )),
            )
        })
        .collect()
}

/// obtain map from local id to living range
#[must_use]
pub fn drop_range(
    datafrog: &PoloniusOutput,
    location_table: &PoloniusLocationTable,
    basic_blocks: &[MirBasicBlock],
) -> HashMap<Local, Vec<Range>> {
    get_range(
        datafrog
            .var_drop_live_on_entry
            .iter()
            .map(|(p, v)| (*p, v.iter().copied())),
        location_table,
        basic_blocks,
    )
}

pub fn get_range(
    live_on_entry: impl Iterator<Item = (impl Idx, impl Iterator<Item = impl Idx>)>,
    location_table: &PoloniusLocationTable,
    basic_blocks: &[MirBasicBlock],
) -> HashMap<Local, Vec<Range>> {
    let mut local_locs = HashMap::new();
    for (loc_idx, locals) in live_on_entry {
        let location = location_table.to_rich_location(loc_idx.index().into());
        for local in locals {
            local_locs
                .entry(local.index())
                .or_insert_with(Vec::new)
                .push(location);
        }
    }
    local_locs
        .into_par_iter()
        .map(|(local, locations)| {
            (
                local.into(),
                range_ops::eliminated_ranges(rich_locations_to_ranges(basic_blocks, &locations)),
            )
        })
        .collect()
}
