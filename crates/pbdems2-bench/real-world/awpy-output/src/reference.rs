// Reference projection from Awpy 1abb386c2e88, retained for paired benchmarks.
use awpy::PlayerState;
use polars::prelude::*;
fn df_from_columns(columns: Vec<Column>) -> PolarsResult<DataFrame> {
    DataFrame::new(columns.first().map_or(0, Column::len), columns)
}
macro_rules! col {
    ($name:literal, $items:expr, $f:expr) => {
        Column::new($name.into(), $items.iter().map($f).collect::<Vec<_>>())
    };
}
pub fn states_to_frame(states: &[PlayerState]) -> PolarsResult<DataFrame> {
    df_from_columns(vec![
        col!("tick", states, |s| s.tick),
        col!("steamid", states, |s| s.steamid),
        col!("name", states, |s| s.name.as_deref()),
        col!("side", states, |s| s.side),
        col!("x", states, |s| s.x),
        col!("y", states, |s| s.y),
        col!("z", states, |s| s.z),
        col!("velocity_x", states, |s| s.velocity_x),
        col!("velocity_y", states, |s| s.velocity_y),
        col!("velocity_z", states, |s| s.velocity_z),
        col!("velocity", states, |s| s.velocity),
        col!("pitch", states, |s| s.pitch),
        col!("yaw", states, |s| s.yaw),
        col!("health", states, |s| s.health),
        col!("armor", states, |s| s.armor),
        col!("has_helmet", states, |s| s.has_helmet),
        col!("has_defuser", states, |s| s.has_defuser),
        col!("has_bomb", states, |s| s.has_bomb),
        col!("active_weapon", states, |s| s.active_weapon),
        col!("primary_weapon", states, |s| s.primary_weapon),
        col!("secondary_weapon", states, |s| s.secondary_weapon),
        col!("fire_grenades", states, |s| s.fire_grenades),
        col!("smoke_grenades", states, |s| s.smoke_grenades),
        col!("he_grenades", states, |s| s.he_grenades),
        col!("flashbangs", states, |s| s.flashbangs),
        col!("decoy_grenades", states, |s| s.decoy_grenades),
        col!("equipment_value", states, |s| s.equipment_value),
        col!("equipment_value_round_start", states, |s| s
            .equipment_value_round_start),
        col!("money", states, |s| s.money),
        col!("is_crouched", states, |s| s.is_crouched),
        col!("is_walking", states, |s| s.is_walking),
        col!("is_jumping", states, |s| s.is_jumping),
        col!("is_in_bomb_zone", states, |s| s.is_in_bomb_zone),
        col!("is_scoped", states, |s| s.is_scoped),
        col!("is_defusing", states, |s| s.is_defusing),
        col!("is_blinded", states, |s| s.is_blinded),
        col!("flash_duration", states, |s| s.flash_duration),
        col!("inventory", states, |s| s.inventory.clone()),
    ])
}
