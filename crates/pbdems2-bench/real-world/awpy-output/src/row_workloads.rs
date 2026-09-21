//! Benchmark-only models of Awpy's remaining snapshot construction costs.
//!
//! Inventory work starts after handle resolution. It uses the real weapon
//! lookup and mirrors datasets::fill_loadout, but doesn't traverse entities.
//! Prebound metadata is a best-case control, not an implemented class-ID cache.
//! Row controls drain borrowed fixture storage so its allocation can be dropped
//! outside the timer. Only production-shaped temporary row buffers are timed.
use crate::columns::SnapshotColumns;
use awpy::PlayerState;
use awpy::weapons::{WeaponInfo, WeaponSlot, weapon_info};

pub type RowDispatch = fn(&mut Vec<PlayerState>, usize) -> SnapshotColumns;

pub const LOADOUTS: &[(&str, &[Option<&str>])] = &[
    ("empty", &[]),
    ("pistol", &[Some("CWeaponGlock"), Some("CKnife")]),
    (
        "full",
        &[
            Some("CWeaponM4A1Silencer"),
            Some("CWeaponUSPSilencer"),
            Some("CKnife"),
            Some("CFlashbang"),
            Some("CFlashbang"),
            Some("CSmokeGrenade"),
            Some("CIncendiaryGrenade"),
        ],
    ),
    (
        "sparse",
        &[
            None,
            Some("UnknownClass"),
            Some("CKnifeGG"),
            None,
            Some("CWeaponTaser"),
            Some("CC4"),
        ],
    ),
];

pub fn loadout(classes: &[Option<&str>], capacity_hint: usize) -> PlayerState {
    let infos = classes.iter().filter_map(|class| weapon_info((*class)?));
    build_loadout(infos, capacity_hint)
}

pub fn prebound_loadout(infos: &[WeaponInfo], capacity_hint: usize) -> PlayerState {
    build_loadout(infos.iter().copied(), capacity_hint)
}

fn build_loadout(infos: impl Iterator<Item = WeaponInfo>, capacity_hint: usize) -> PlayerState {
    let mut state = PlayerState::default();
    for info in infos {
        if state.inventory.is_empty() {
            // Lazy reservation keeps empty and unrecognized loadouts allocation-free.
            state.inventory.reserve(capacity_hint);
        } else {
            state.inventory.push(',');
        }
        state.inventory.push_str(info.name);
        match info.slot {
            WeaponSlot::Primary => state.primary_weapon = Some(info.name),
            WeaponSlot::Secondary => state.secondary_weapon = Some(info.name),
            WeaponSlot::Grenade => match info.name {
                "hegrenade" => state.he_grenades += 1,
                "flashbang" => state.flashbangs += 1,
                "smokegrenade" => state.smoke_grenades += 1,
                "molotov" | "incendiary" => state.fire_grenades += 1,
                "decoy" => state.decoy_grenades += 1,
                _ => {}
            },
            WeaponSlot::C4 => state.has_bomb = true,
            WeaponSlot::Melee | WeaponSlot::Equipment => {}
        }
    }
    state
}

/// Model the current fresh, unknown-length row vector per tick.
pub fn fresh_tick_rows(states: &mut Vec<PlayerState>, players: usize) -> SnapshotColumns {
    assert!(players > 0);
    let mut states = states.drain(..);
    let mut columns = SnapshotColumns::default();
    while !states.as_slice().is_empty() {
        let mut tick = Vec::new();
        // Don't collect an exact-size iterator: production discovers rows incrementally.
        for state in states.by_ref().take(players) {
            tick.push(state);
        }
        columns.extend(tick);
    }
    columns
}

/// Model reusing the temporary allocation while retaining owned row delivery.
pub fn reused_tick_rows(states: &mut Vec<PlayerState>, players: usize) -> SnapshotColumns {
    assert!(players > 0);
    let mut states = states.drain(..);
    let mut columns = SnapshotColumns::default();
    let mut tick = Vec::new();
    while !states.as_slice().is_empty() {
        for state in states.by_ref().take(players) {
            tick.push(state);
        }
        columns.extend(tick.drain(..));
    }
    columns
}

/// Best-case dispatch control without temporary per-tick row storage.
pub fn direct_tick_rows(states: &mut Vec<PlayerState>, players: usize) -> SnapshotColumns {
    assert!(players > 0);
    let mut states = states.drain(..);
    let mut columns = SnapshotColumns::default();
    while !states.as_slice().is_empty() {
        columns.extend(states.by_ref().take(players));
    }
    columns
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{columns_owned, reference_owned, rows};

    #[test]
    fn loadout_controls_preserve_slot_order_duplicates_and_all_inventory_fields() {
        let classes = [
            None,
            Some("UnknownClass"),
            Some("CAK47"),
            Some("CWeaponAWP"),
            Some("CWeaponGlock"),
            Some("CKnifeGG"),
            Some("CFlashbang"),
            Some("CFlashbang"),
            Some("CHEGrenade"),
            Some("CSmokeGrenade"),
            Some("CMolotovGrenade"),
            Some("CIncendiaryGrenade"),
            Some("CDecoyGrenade"),
            Some("CC4"),
            Some("CWeaponTaser"),
        ];
        let infos: Vec<_> = classes
            .iter()
            .filter_map(|class| weapon_info((*class)?))
            .collect();
        let expected = loadout(&classes, 0);
        assert_eq!(
            expected.inventory,
            "ak47,awp,glock,knife,flashbang,flashbang,hegrenade,smokegrenade,molotov,incendiary,decoy,c4,taser"
        );
        assert_eq!(expected.primary_weapon, Some("awp"));
        assert_eq!(expected.secondary_weapon, Some("glock"));
        assert_eq!(
            (
                expected.flashbangs,
                expected.he_grenades,
                expected.smoke_grenades,
                expected.fire_grenades,
                expected.decoy_grenades
            ),
            (2, 1, 1, 2, 1)
        );
        assert!(expected.has_bomb);
        let expected = reference_owned(vec![expected]).unwrap();
        for actual in [loadout(&classes, 64), prebound_loadout(&infos, 64)] {
            assert!(
                columns_owned(vec![actual])
                    .unwrap()
                    .equals_missing(&expected)
            );
        }
        for hint in [0, 64] {
            assert_eq!(
                loadout(&[None, Some("UnknownClass")], hint)
                    .inventory
                    .capacity(),
                0
            );
        }
    }

    #[test]
    fn dispatch_controls_preserve_all_columns_and_partial_final_ticks() {
        for count in [0, 1, 10, 11, 37] {
            for players in [1, 10, 12] {
                let states = rows(count);
                let expected = reference_owned(states.clone()).unwrap();
                for dispatch in [
                    fresh_tick_rows as RowDispatch,
                    reused_tick_rows,
                    direct_tick_rows,
                ] {
                    let mut input = states.clone();
                    let capacity = input.capacity();
                    let columns = dispatch(&mut input, players);
                    assert!(input.is_empty());
                    assert_eq!(input.capacity(), capacity);
                    let actual = columns.into_frame().unwrap();
                    assert_eq!(actual.schema(), expected.schema());
                    assert!(actual.equals_missing(&expected));
                }
            }
        }
    }
}
