// Private replay-parsing control. See PARSING.md for workload and validation limits.
use serde_json::{Value, json};
use source2_demo::DemoRunner;
use source2_demo::prelude::{Context, Interests, Observer, ObserverResult};
use std::{
    collections::{BTreeMap, HashSet, hash_map::DefaultHasher},
    hash::{Hash, Hasher},
    time::Instant,
};

const SAMPLE_TICKS: [i32; 5] = [10000, 26147, 40441, 187554, 200000];
const PROPERTIES: [&str; 5] = [
    "m_iHealth",
    "m_iMaxHealth",
    "m_hPawn",
    "m_hPlayerPawn",
    "m_steamID",
];
#[derive(Default)]
struct Counts {
    ticks: usize,
    first: Option<i32>,
    last: i32,
}
impl Counts {
    fn tick(&mut self, tick: i32) {
        self.ticks += 1;
        self.first.get_or_insert(tick);
        self.last = tick;
    }
}
fn hash(value: impl Hash) -> u64 {
    let mut h = DefaultHasher::new();
    value.hash(&mut h);
    h.finish()
}
fn table_row(key: Option<&str>, data: Option<&[u8]>) -> Value {
    json!([key.unwrap_or_default(), data.map(hash)])
}
fn core_sample(ctx: &pbdems2::ParserState) -> Value {
    let mut entities = BTreeMap::new();
    let mut fields = BTreeMap::new();
    let mut properties = BTreeMap::new();
    for (index, e) in ctx.entities().iter() {
        entities.insert(index, json!([e.serial, e.class_name]));
        fields.insert(
            index,
            e.fields
                .iter()
                .map(|(k, v)| (*k, format!("{v:?}")))
                .collect::<BTreeMap<_, _>>(),
        );
        if e.class_name.as_ref() == "CCitadelPlayerPawn"
            || e.class_name.as_ref() == "CCitadelPlayerController"
        {
            let serializer = ctx.serializers().get(&e.class_name).unwrap();
            properties.insert(
                index,
                PROPERTIES
                    .into_iter()
                    .filter_map(|name| {
                        e.get_by_name(name, serializer)
                            .map(|v| (name, format!("{v:?}")))
                    })
                    .collect::<BTreeMap<_, _>>(),
            );
        }
    }
    let tables = ctx
        .string_tables()
        .tables()
        .iter()
        .map(|t| {
            (
                t.name(),
                t.entries()
                    .iter()
                    .map(|r| table_row(r.string.as_deref(), r.user_data.as_deref()))
                    .collect::<Vec<_>>(),
            )
        })
        .collect::<BTreeMap<_, _>>();
    json!({"entities":entities,"properties":properties,"tables":tables,"field_hash":hash(serde_json::to_vec(&fields).unwrap())})
}
struct SourceObserver {
    counts: Counts,
    all_tables: bool,
    verify: bool,
    samples: BTreeMap<i32, Value>,
}
impl Observer for SourceObserver {
    fn interests(&self) -> Interests {
        Interests::ENTITY_STATE
            | Interests::TICK_END
            | if self.all_tables {
                Interests::STRING_TABLE_STATE
            } else {
                Interests::empty()
            }
    }
    fn on_tick_end(&mut self, ctx: &Context) -> ObserverResult {
        let tick = ctx.tick() as i32;
        self.counts.tick(tick);
        if self.verify && SAMPLE_TICKS.contains(&tick) {
            let entities = ctx
                .entities()
                .iter()
                .map(|e| (e.index(), json!([e.serial(), e.class().name()])))
                .collect::<BTreeMap<_, _>>();
            let properties = ctx
                .entities()
                .iter()
                .filter(|e| {
                    matches!(
                        e.class().name(),
                        "CCitadelPlayerPawn" | "CCitadelPlayerController"
                    )
                })
                .map(|e| {
                    (
                        e.index(),
                        PROPERTIES
                            .into_iter()
                            .filter_map(|name| {
                                e.get_property(name).ok().map(|v| (name, format!("{v:?}")))
                            })
                            .collect::<BTreeMap<_, _>>(),
                    )
                })
                .collect::<BTreeMap<_, _>>();
            let tables = ctx
                .string_tables()
                .iter()
                .map(|t| {
                    (
                        t.name(),
                        t.iter()
                            .map(|r| table_row(Some(r.key()), r.value()))
                            .collect::<Vec<_>>(),
                    )
                })
                .collect::<BTreeMap<_, _>>();
            self.samples.insert(
                tick,
                json!({"entities":entities,"properties":properties,"tables":tables}),
            );
        }
        Ok(())
    }
}
fn main() -> Result<(), Box<dyn std::error::Error>> {
    let args = std::env::args().collect::<Vec<_>>();
    if args.len() < 4 {
        return Err(
            "usage: matched-parser boon|awpy|source2 all|players|baseline-only DEMO [--verify]"
                .into(),
        );
    }
    let bytes = std::fs::read(&args[3])?;
    let verify = args.get(4).is_some_and(|v| v == "--verify");
    let mut counts = Counts::default();
    let mut samples = BTreeMap::new();
    let start = Instant::now();
    match args[1].as_str() {
        "boon" => {
            let parser = boon_parser::Parser::from_bytes(bytes);
            let mut visit = |ctx: &pbdems2::ParserState| {
                counts.tick(ctx.tick());
                if verify && SAMPLE_TICKS.contains(&ctx.tick()) {
                    samples.insert(ctx.tick(), core_sample(ctx));
                }
            };
            match args[2].as_str() {
                "all" => parser.run_to_end(&mut visit)?,
                "players" => parser.run_to_end_filtered(
                    &HashSet::from(["CCitadelPlayerController", "CCitadelPlayerPawn"]),
                    &mut visit,
                )?,
                _ => return Err("expected all or players for boon".into()),
            }
        }
        "awpy" => {
            let parser = awpy::Parser::from_bytes(bytes);
            let mut visit = |ctx: &pbdems2::ParserState| {
                counts.tick(ctx.tick());
                if verify && SAMPLE_TICKS.contains(&ctx.tick()) {
                    samples.insert(ctx.tick(), core_sample(ctx));
                }
            };
            match args[2].as_str() {
                "all" => parser.run_to_end(&mut visit)?,
                "players" => parser.run_to_end_filtered(
                    &HashSet::from(["CCSPlayerController", "CCSPlayerPawn"]),
                    &mut visit,
                )?,
                _ => return Err("expected all or players for awpy".into()),
            }
        }
        "source2" => {
            if !matches!(args[2].as_str(), "all" | "baseline-only") {
                return Err("source2 comparison requires all or baseline-only".into());
            }
            let mut parser = source2_demo::Parser::from_slice(&bytes)?;
            let observer = parser.add_observer(SourceObserver {
                counts: Counts::default(),
                all_tables: args[2] == "all",
                verify,
                samples: BTreeMap::new(),
            });
            parser.run_to_end()?;
            let mut observer = observer.borrow_mut();
            counts = std::mem::take(&mut observer.counts);
            samples = std::mem::take(&mut observer.samples);
            drop(observer);
            drop(parser);
            drop(bytes);
        }
        _ => return Err("unknown parser".into()),
    }
    let seconds = start.elapsed().as_secs_f64();
    println!(
        "{}",
        json!({"parser":args[1],"mode":args[2],"seconds":seconds,"ticks":counts.ticks,"first_tick":counts.first,"last_tick":counts.last,"verification":verify,"samples":samples})
    );
    Ok(())
}
