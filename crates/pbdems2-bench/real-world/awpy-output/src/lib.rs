#[path = "../../../../../../awpy-github/awpy/crates/awpy-python/src/snapshot_columns.rs"]
pub mod columns;
pub mod reference;
pub mod row_workloads;

use awpy::PlayerState;
use polars::prelude::*;

/// Production-style conversion including destruction of the owned row buffer.
pub fn reference_owned(states: Vec<PlayerState>) -> PolarsResult<DataFrame> {
    reference::states_to_frame(&states)
}

pub fn columns_owned(states: Vec<PlayerState>) -> PolarsResult<DataFrame> {
    states
        .into_iter()
        .collect::<columns::SnapshotColumns>()
        .into_frame()
}

/// Unknown-length accumulation, as used by per-tick callbacks.
pub fn columns_growing(states: Vec<PlayerState>) -> PolarsResult<DataFrame> {
    let mut columns = columns::SnapshotColumns::default();
    columns.extend(states);
    columns.into_frame()
}

/// Deterministic rows with nulls, strings, inventory, and all snapshot fields.
pub fn rows(count: usize) -> Vec<PlayerState> {
    (0..count)
        .map(|i| PlayerState {
            tick: (i / 10) as i32,
            steamid: Some(76561198000000000 + (i % 10) as u64),
            name: (i % 11 != 0).then(|| format!("player-{}", i % 10)),
            side: Some(if i % 10 < 5 {
                "terrorist"
            } else {
                "counter-terrorist"
            }),
            x: Some((i % 128) as f32),
            y: Some(-17.25),
            z: None,
            velocity_x: Some(125.5),
            velocity_y: Some(-60.0),
            velocity_z: None,
            velocity: Some(139.113),
            pitch: -30.0,
            yaw: 92.0,
            health: 100,
            armor: 50,
            has_helmet: true,
            has_defuser: i % 2 == 0,
            has_bomb: i % 10 == 0,
            active_weapon: Some("ak47"),
            primary_weapon: Some("ak47"),
            secondary_weapon: None,
            fire_grenades: 1,
            smoke_grenades: 1,
            he_grenades: 0,
            flashbangs: 2,
            decoy_grenades: 0,
            equipment_value: 4200,
            equipment_value_round_start: 5000,
            money: 800,
            is_crouched: false,
            is_walking: true,
            is_jumping: false,
            is_in_bomb_zone: i % 2 == 0,
            is_scoped: false,
            is_defusing: false,
            is_blinded: true,
            flash_duration: 3.5,
            inventory: "ak47,knife,smokegrenade,flashbang,flashbang".to_owned(),
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    use columns::SnapshotColumns;

    #[test]
    fn owned_columns_preserve_all_38_columns_types_nulls_values_and_order() {
        for count in [0, 1, 37, 1000] {
            let rows = rows(count);
            let expected = reference_owned(rows.clone()).unwrap();
            let actual = columns_owned(rows).unwrap();
            assert_eq!(expected.schema(), actual.schema());
            assert_eq!(actual.width(), 38);
            assert!(actual.equals_missing(&expected));
        }
    }

    #[test]
    fn appending_chunk_columns_matches_whole_row_conversion() {
        let rows = rows(37);
        let expected = reference_owned(rows.clone()).unwrap();
        let mut columns = SnapshotColumns::default();
        for chunk in rows.chunks(7) {
            columns.append(chunk.iter().cloned().collect());
        }
        assert!(columns.into_frame().unwrap().equals_missing(&expected));
    }

    #[test]
    fn column_builders_preserve_unicode_empty_strings_and_nonfinite_floats() {
        let mut rows = rows(3);
        rows[0].name = Some("玩家".into());
        rows[1].name = Some(String::new());
        rows[0].inventory.clear();
        rows[0].pitch = f32::NAN;
        rows[1].x = Some(f32::INFINITY);
        rows[2].yaw = f32::NEG_INFINITY;
        let expected = reference_owned(rows.clone()).unwrap();
        assert!(
            columns_owned(rows.clone())
                .unwrap()
                .equals_missing(&expected)
        );
        assert!(columns_growing(rows).unwrap().equals_missing(&expected));
    }
}
