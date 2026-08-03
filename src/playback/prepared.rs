//! Reusable signon checkpoints and independent playback sessions.

use std::collections::HashSet;

use crate::demo::DemoIndex;
use crate::error::Error;
use crate::limits::DecodeLimits;

use super::{DemoAdapter, DemoParser, ParserState};

/// A game adapter that can save semantic signon state and restore a fresh run.
///
/// Checkpoints should omit transient allocations such as packet scratch
/// buffers and per-tick output. This lets one prepared playback seed create
/// inexpensive, isolated sessions for repeated or parallel decoding.
pub trait CheckpointAdapter: DemoAdapter + Sized {
    /// Semantic adapter state required to continue immediately after signon.
    type Checkpoint;

    /// Capture the adapter state at the end of signon.
    fn checkpoint(&self) -> Self::Checkpoint;

    /// Construct an independent adapter from a signon checkpoint.
    fn from_checkpoint(checkpoint: &Self::Checkpoint) -> Self;
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct DemoIdentity {
    address: usize,
    length: usize,
    limits: DecodeLimits,
}

impl DemoIdentity {
    fn new(parser: &DemoParser<'_>) -> Self {
        let data = parser.demo.data();
        Self {
            address: data.as_ptr() as usize,
            length: data.len(),
            limits: parser.demo.limits(),
        }
    }

    fn matches(&self, parser: &DemoParser<'_>) -> bool {
        *self == Self::new(parser)
    }
}

/// Decoded signon state and seek metadata reusable across playback runs.
///
/// This value does not borrow the encoded demo, so an owning Awpy or Boon
/// parser can cache it without becoming self-referential. A session validates
/// that its borrowed DemoParser views the same allocation with the same decode
/// limits.
///
/// The value is automatically Send and Sync when the adapter checkpoint and
/// neutral parser state are, allowing independent sessions to be created from
/// scoped worker threads.
pub struct PreparedPlayback<A: CheckpointAdapter> {
    initial_state: ParserState,
    adapter_checkpoint: A::Checkpoint,
    index: DemoIndex,
    identity: DemoIdentity,
}

impl<A: CheckpointAdapter> PreparedPlayback<A> {
    /// Neutral parser state captured immediately after signon.
    pub const fn initial_state(&self) -> &ParserState {
        &self.initial_state
    }

    /// Header-only index built during preparation.
    pub const fn index(&self) -> &DemoIndex {
        &self.index
    }

    /// Create an isolated one-run session over the original demo allocation.
    pub fn session<'demo, 'prepared>(
        &'prepared self,
        parser: DemoParser<'demo>,
    ) -> std::result::Result<PlaybackSession<'demo, 'prepared, A>, A::Error> {
        if !self.identity.matches(&parser) {
            return Err(A::Error::from(Error::Parse {
                context: "prepared playback belongs to a different demo buffer or decode limits"
                    .into(),
            }));
        }

        Ok(PlaybackSession {
            parser,
            prepared: self,
            state: self.initial_state.clone(),
            adapter: A::from_checkpoint(&self.adapter_checkpoint),
        })
    }
}

/// One independent playback run restored from a PreparedPlayback seed.
///
/// Playback methods consume the session so its mutable entity, string-table,
/// and adapter state cannot accidentally leak into another run.
pub struct PlaybackSession<'demo, 'prepared, A: CheckpointAdapter> {
    parser: DemoParser<'demo>,
    prepared: &'prepared PreparedPlayback<A>,
    state: ParserState,
    adapter: A,
}

impl<'demo, A: CheckpointAdapter> PlaybackSession<'demo, '_, A> {
    /// Initial session state before playback begins.
    pub const fn state(&self) -> &ParserState {
        &self.state
    }

    /// Restored game adapter for this session.
    pub const fn adapter(&self) -> &A {
        &self.adapter
    }

    /// Mutably configure or inspect the restored game adapter before playback.
    pub const fn adapter_mut(&mut self) -> &mut A {
        &mut self.adapter
    }

    /// Decode every post-signon command with an infallible state callback.
    pub fn run_to_end<F>(self, on_tick: F) -> std::result::Result<ParserState, A::Error>
    where
        F: FnMut(&ParserState),
    {
        let mut on_tick = on_tick;
        self.try_run_to_end(|state| {
            on_tick(state);
            Ok(())
        })
    }

    /// Decode every post-signon command with a fallible state callback.
    pub fn try_run_to_end<F>(self, on_tick: F) -> std::result::Result<ParserState, A::Error>
    where
        F: FnMut(&ParserState) -> std::result::Result<(), A::Error>,
    {
        let mut on_tick = on_tick;
        self.try_run_to_end_with_adapter(|state, _| on_tick(state))
    }

    /// Decode every command and expose mutable adapter state at each tick.
    pub fn run_to_end_with_adapter<F>(
        self,
        on_tick: F,
    ) -> std::result::Result<ParserState, A::Error>
    where
        F: FnMut(&ParserState, &mut A),
    {
        let mut on_tick = on_tick;
        self.try_run_to_end_with_adapter(|state, adapter| {
            on_tick(state, adapter);
            Ok(())
        })
    }

    /// Fallible full playback with mutable adapter access at each tick.
    pub fn try_run_to_end_with_adapter<F>(
        mut self,
        on_tick: F,
    ) -> std::result::Result<ParserState, A::Error>
    where
        F: FnMut(&ParserState, &mut A) -> std::result::Result<(), A::Error>,
    {
        if let Some(start) = self.prepared.index.stream_start() {
            self.parser.replay(
                &mut self.adapter,
                &mut self.state,
                start,
                None,
                None,
                on_tick,
            )?;
        }
        Ok(self.state)
    }

    /// Decode every command while materializing only filtered entity classes.
    pub fn run_to_end_filtered<F>(
        self,
        class_filter: &HashSet<&str>,
        on_tick: F,
    ) -> std::result::Result<ParserState, A::Error>
    where
        F: FnMut(&ParserState),
    {
        let mut on_tick = on_tick;
        self.try_run_to_end_filtered(class_filter, |state| {
            on_tick(state);
            Ok(())
        })
    }

    /// Fallible filtered playback with a state-only callback.
    pub fn try_run_to_end_filtered<F>(
        self,
        class_filter: &HashSet<&str>,
        on_tick: F,
    ) -> std::result::Result<ParserState, A::Error>
    where
        F: FnMut(&ParserState) -> std::result::Result<(), A::Error>,
    {
        let mut on_tick = on_tick;
        self.try_run_to_end_filtered_with_adapter(class_filter, |state, _| on_tick(state))
    }

    /// Filter entities and expose mutable adapter state at each tick.
    pub fn run_to_end_filtered_with_adapter<F>(
        self,
        class_filter: &HashSet<&str>,
        on_tick: F,
    ) -> std::result::Result<ParserState, A::Error>
    where
        F: FnMut(&ParserState, &mut A),
    {
        let mut on_tick = on_tick;
        self.try_run_to_end_filtered_with_adapter(class_filter, |state, adapter| {
            on_tick(state, adapter);
            Ok(())
        })
    }

    /// Fallible filtered playback with mutable adapter access at each tick.
    pub fn try_run_to_end_filtered_with_adapter<F>(
        mut self,
        class_filter: &HashSet<&str>,
        on_tick: F,
    ) -> std::result::Result<ParserState, A::Error>
    where
        F: FnMut(&ParserState, &mut A) -> std::result::Result<(), A::Error>,
    {
        if let Some(start) = self.prepared.index.stream_start() {
            self.parser.replay(
                &mut self.adapter,
                &mut self.state,
                start,
                None,
                Some(class_filter),
                on_tick,
            )?;
        }
        Ok(self.state)
    }

    /// Seek from the nearest full packet and replay through target_tick.
    pub fn parse_to_tick(mut self, target_tick: i32) -> std::result::Result<ParserState, A::Error> {
        let start = self
            .prepared
            .index
            .full_packet_at_or_before(target_tick)
            .map(|position| position.offset())
            .or(self.prepared.index.stream_start());
        if let Some(start) = start {
            self.parser.replay(
                &mut self.adapter,
                &mut self.state,
                start,
                Some(target_tick),
                None,
                |_, _| Ok(()),
            )?;
        }
        Ok(self.state)
    }

    /// Decode one segment with an infallible state callback.
    ///
    /// A full-packet restart is exact only for classes the game fully
    /// re-keyframes in that snapshot. The consumer must choose a compatible
    /// class_filter.
    pub fn decode_segment<F>(
        self,
        start: Option<usize>,
        end_tick: i32,
        class_filter: &HashSet<&str>,
        on_tick: F,
    ) -> std::result::Result<ParserState, A::Error>
    where
        F: FnMut(&ParserState),
    {
        let mut on_tick = on_tick;
        self.try_decode_segment(start, end_tick, class_filter, |state| {
            on_tick(state);
            Ok(())
        })
    }

    /// Decode one segment with a fallible state callback.
    pub fn try_decode_segment<F>(
        self,
        start: Option<usize>,
        end_tick: i32,
        class_filter: &HashSet<&str>,
        on_tick: F,
    ) -> std::result::Result<ParserState, A::Error>
    where
        F: FnMut(&ParserState) -> std::result::Result<(), A::Error>,
    {
        let mut on_tick = on_tick;
        self.try_decode_segment_with_adapter(start, end_tick, class_filter, |state, _| {
            on_tick(state)
        })
    }

    /// Decode one segment and expose mutable adapter state at each tick.
    pub fn decode_segment_with_adapter<F>(
        self,
        start: Option<usize>,
        end_tick: i32,
        class_filter: &HashSet<&str>,
        on_tick: F,
    ) -> std::result::Result<ParserState, A::Error>
    where
        F: FnMut(&ParserState, &mut A),
    {
        let mut on_tick = on_tick;
        self.try_decode_segment_with_adapter(start, end_tick, class_filter, |state, adapter| {
            on_tick(state, adapter);
            Ok(())
        })
    }

    /// Fallible segment playback with mutable adapter access at each tick.
    pub fn try_decode_segment_with_adapter<F>(
        mut self,
        start: Option<usize>,
        end_tick: i32,
        class_filter: &HashSet<&str>,
        on_tick: F,
    ) -> std::result::Result<ParserState, A::Error>
    where
        F: FnMut(&ParserState, &mut A) -> std::result::Result<(), A::Error>,
    {
        if let Some(start) = start.or(self.prepared.index.stream_start()) {
            self.parser.replay(
                &mut self.adapter,
                &mut self.state,
                start,
                Some(end_tick.saturating_sub(1)),
                Some(class_filter),
                on_tick,
            )?;
        }
        Ok(self.state)
    }
}

impl<'demo> DemoParser<'demo> {
    /// Decode signon and build a reusable playback seed.
    ///
    /// The adapter is consumed so its semantic state can be checkpointed at
    /// DEM_SyncTick without requiring it or its scratch buffers to be cloned.
    pub fn prepare<A: CheckpointAdapter>(
        &self,
        mut adapter: A,
        default_tick_interval: f32,
    ) -> std::result::Result<PreparedPlayback<A>, A::Error> {
        let (initial_state, stream_start) = self.initialize(&mut adapter, default_tick_interval)?;
        let index = self.demo.index().map_err(A::Error::from)?;
        if stream_start != index.stream_start() {
            return Err(A::Error::from(Error::Parse {
                context: "signon stream start did not match the command index".into(),
            }));
        }

        Ok(PreparedPlayback {
            initial_state,
            adapter_checkpoint: adapter.checkpoint(),
            index,
            identity: DemoIdentity::new(self),
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::Result;
    use crate::demo::{CommandFrame, HEADER_SIZE, MAGIC, command};
    use crate::entity::{
        BareCharEncoding, ClassEntry, DecodeProfile, FlattenedSerializer,
        FlattenedSerializerDefinition, PreciseQAngleMode,
    };
    use crate::playback::CommandContext;

    #[derive(Debug, Default)]
    struct StatefulAdapter {
        signon_marker: u32,
        scratch: Vec<u8>,
        tick_messages: Vec<i32>,
    }

    impl DemoAdapter for StatefulAdapter {
        type Error = Error;

        fn handle_command(
            &mut self,
            frame: &CommandFrame<'_>,
            _body: &[u8],
            context: &mut CommandContext<'_, '_>,
        ) -> Result<()> {
            match frame.header().cmd {
                command::SEND_TABLES => context.install_serializers(
                    FlattenedSerializer {
                        serializers: vec![FlattenedSerializerDefinition {
                            serializer_name_sym: Some(0),
                            fields_index: vec![],
                        }],
                        symbols: vec!["CTest".into()],
                        fields: vec![],
                    },
                    DecodeProfile::new(
                        BareCharEncoding::NullTerminatedString,
                        PreciseQAngleMode::Centered,
                    ),
                ),
                command::CLASS_INFO => {
                    context.install_class_info([ClassEntry::new(0, "CTest", "CTest")])
                }
                command::SIGNON_PACKET => {
                    self.signon_marker = 7;
                    self.scratch.resize(1024, 0x5a);
                    Ok(())
                }
                command::PACKET | command::FULL_PACKET => {
                    self.tick_messages.push(frame.header().tick);
                    self.scratch.push(frame.header().tick as u8);
                    Ok(())
                }
                _ => Ok(()),
            }
        }
    }

    impl CheckpointAdapter for StatefulAdapter {
        type Checkpoint = u32;

        fn checkpoint(&self) -> Self::Checkpoint {
            self.signon_marker
        }

        fn from_checkpoint(checkpoint: &Self::Checkpoint) -> Self {
            Self {
                signon_marker: *checkpoint,
                scratch: Vec::new(),
                tick_messages: Vec::new(),
            }
        }
    }

    fn push_command(bytes: &mut Vec<u8>, cmd: i32, tick: u8) {
        bytes.extend_from_slice(&[cmd as u8, tick, 0]);
    }

    fn fixture() -> Vec<u8> {
        let mut bytes = Vec::from(MAGIC);
        bytes.resize(HEADER_SIZE, 0);
        push_command(&mut bytes, command::SEND_TABLES, 0);
        push_command(&mut bytes, command::CLASS_INFO, 0);
        push_command(&mut bytes, command::SIGNON_PACKET, 0);
        push_command(&mut bytes, command::SYNC_TICK, 0);
        push_command(&mut bytes, command::PACKET, 1);
        push_command(&mut bytes, command::FULL_PACKET, 2);
        push_command(&mut bytes, command::PACKET, 3);
        push_command(&mut bytes, command::STOP, 4);
        bytes
    }

    fn assert_send_sync<T: Send + Sync>() {}

    #[test]
    fn preparation_captures_signon_and_index_without_borrowing() {
        assert_send_sync::<PreparedPlayback<StatefulAdapter>>();

        let bytes = fixture();
        let parser = DemoParser::new(&bytes).expect("valid demo");
        let prepared = parser
            .prepare(StatefulAdapter::default(), 1.0 / 30.0)
            .expect("valid preparation");

        assert!(prepared.initial_state().is_initialized());
        assert_eq!(prepared.initial_state().tick_interval(), 1.0 / 30.0);
        assert_eq!(prepared.index().distinct_ticks(), [1, 2, 3]);
        assert_eq!(prepared.index().full_packets().len(), 1);

        let session = prepared.session(parser).expect("matching demo");
        assert_eq!(session.adapter().signon_marker, 7);
        assert!(session.adapter().scratch.is_empty());
    }

    #[test]
    fn prepared_and_cold_playback_match() {
        let bytes = fixture();
        let parser = DemoParser::new(&bytes).expect("valid demo");

        let mut cold_ticks = Vec::new();
        let cold = parser
            .run_to_end(&mut StatefulAdapter::default(), 1.0 / 30.0, |state| {
                cold_ticks.push(state.tick())
            })
            .expect("cold playback");

        let prepared = parser
            .prepare(StatefulAdapter::default(), 1.0 / 30.0)
            .expect("valid preparation");
        let mut prepared_ticks = Vec::new();
        let restored = prepared
            .session(parser)
            .expect("matching demo")
            .run_to_end(|state| prepared_ticks.push(state.tick()))
            .expect("prepared playback");

        assert_eq!(prepared_ticks, cold_ticks);
        assert_eq!(restored.tick(), cold.tick());

        let cold_seek = parser
            .parse_to_tick(&mut StatefulAdapter::default(), 1.0 / 30.0, 2)
            .expect("cold seek");
        let prepared_seek = prepared
            .session(parser)
            .expect("matching demo")
            .parse_to_tick(2)
            .expect("prepared seek");
        assert_eq!(prepared_seek.tick(), cold_seek.tick());
    }

    #[test]
    fn prepared_filter_segment_and_callback_failures_match_cold_semantics() {
        let bytes = fixture();
        let parser = DemoParser::new(&bytes).expect("valid demo");
        let prepared = parser
            .prepare(StatefulAdapter::default(), 1.0 / 30.0)
            .expect("valid preparation");
        let filter = HashSet::from(["CTest"]);

        let mut filtered_ticks = Vec::new();
        prepared
            .session(parser)
            .expect("matching demo")
            .run_to_end_filtered(&filter, |state| filtered_ticks.push(state.tick()))
            .expect("filtered playback");
        assert_eq!(filtered_ticks, [1, 2, 3, 4]);

        let start = prepared.index().full_packets()[0].offset();
        let mut segment_ticks = Vec::new();
        let segment = prepared
            .session(parser)
            .expect("matching demo")
            .decode_segment(Some(start), 3, &filter, |state| {
                segment_ticks.push(state.tick());
            })
            .expect("segment playback");
        assert_eq!(segment_ticks, [2]);
        assert_eq!(segment.tick(), 2);

        let mut failed_ticks = Vec::new();
        let error = prepared
            .session(parser)
            .expect("matching demo")
            .try_run_to_end_with_adapter(|state, adapter| {
                failed_ticks.push(state.tick());
                adapter.tick_messages.clear();
                if state.tick() == 2 {
                    return Err(Error::Parse {
                        context: "prepared consumer failed".into(),
                    });
                }
                Ok(())
            })
            .err()
            .expect("callback failure must abort playback");
        assert_eq!(failed_ticks, [1, 2]);
        assert!(matches!(
            error,
            Error::Parse { context } if context == "prepared consumer failed"
        ));
    }

    #[test]
    fn callbacks_can_drain_adapter_state_without_crossing_sessions() {
        let bytes = fixture();
        let parser = DemoParser::new(&bytes).expect("valid demo");
        let prepared = parser
            .prepare(StatefulAdapter::default(), 1.0 / 30.0)
            .expect("valid preparation");

        let mut observed = Vec::new();
        prepared
            .session(parser)
            .expect("matching demo")
            .run_to_end_with_adapter(|state, adapter| {
                assert_eq!(adapter.signon_marker, 7);
                observed.push((state.tick(), std::mem::take(&mut adapter.tick_messages)));
            })
            .expect("prepared playback");
        assert_eq!(
            observed,
            [(1, vec![1]), (2, vec![2]), (3, vec![3]), (4, vec![])]
        );

        let session = prepared.session(parser).expect("independent session");
        assert_eq!(session.adapter().signon_marker, 7);
        assert!(session.adapter().tick_messages.is_empty());
        assert!(session.adapter().scratch.is_empty());
    }

    #[test]
    fn rejects_other_buffers_and_decode_limits() {
        let bytes = fixture();
        let parser = DemoParser::new(&bytes).expect("valid demo");
        let prepared = parser
            .prepare(StatefulAdapter::default(), 1.0 / 30.0)
            .expect("valid preparation");

        let other_bytes = fixture();
        let other = DemoParser::new(&other_bytes).expect("valid other demo");
        assert!(prepared.session(other).is_err());

        let other_limits = DecodeLimits::default().with_max_command_body_bytes(1024);
        let other = DemoParser::with_limits(&bytes, other_limits).expect("valid alternate view");
        assert!(prepared.session(other).is_err());
    }

    #[test]
    fn prepared_sessions_run_independently_in_parallel() {
        let bytes = fixture();
        let parser = DemoParser::new(&bytes).expect("valid demo");
        let prepared = parser
            .prepare(StatefulAdapter::default(), 1.0 / 30.0)
            .expect("valid preparation");

        let results = std::thread::scope(|scope| {
            let handles: Vec<_> = (0..2)
                .map(|_| {
                    let prepared = &prepared;
                    scope.spawn(move || {
                        let mut ticks = Vec::new();
                        prepared
                            .session(parser)
                            .expect("matching demo")
                            .run_to_end(|state| ticks.push(state.tick()))
                            .expect("parallel playback");
                        ticks
                    })
                })
                .collect();
            handles
                .into_iter()
                .map(|handle| handle.join().expect("worker did not panic"))
                .collect::<Vec<_>>()
        });

        assert_eq!(results, [vec![1, 2, 3, 4], vec![1, 2, 3, 4]]);
    }
}
