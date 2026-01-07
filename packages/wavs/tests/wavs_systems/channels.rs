use wavs::{
    dispatcher::DispatcherCommand,
    subsystems::{
        aggregator::AggregatorCommand, engine::EngineCommand, submission::SubmissionCommand,
    },
};

pub struct TestChannels {
    pub subsystem_to_dispatcher_tx: crossbeam::channel::Sender<DispatcherCommand>,
    pub subsystem_to_dispatcher_rx: crossbeam::channel::Receiver<DispatcherCommand>,
    pub dispatcher_to_engine_tx: crossbeam::channel::Sender<EngineCommand>,
    pub dispatcher_to_engine_rx: crossbeam::channel::Receiver<EngineCommand>,
    pub dispatcher_to_submission_tx: crossbeam::channel::Sender<SubmissionCommand>,
    pub dispatcher_to_submission_rx: crossbeam::channel::Receiver<SubmissionCommand>,
    pub dispatcher_to_aggregator_tx: crossbeam::channel::Sender<AggregatorCommand>,
    pub dispatcher_to_aggregator_rx: crossbeam::channel::Receiver<AggregatorCommand>,
}

impl TestChannels {
    pub fn new() -> Self {
        let (subsystem_to_dispatcher_tx, subsystem_to_dispatcher_rx) =
            crossbeam::channel::unbounded::<DispatcherCommand>();

        let (dispatcher_to_engine_tx, dispatcher_to_engine_rx) =
            crossbeam::channel::unbounded::<EngineCommand>();

        let (dispatcher_to_submission_tx, dispatcher_to_submission_rx) =
            crossbeam::channel::unbounded::<SubmissionCommand>();

        let (dispatcher_to_aggregator_tx, dispatcher_to_aggregator_rx) =
            crossbeam::channel::unbounded::<AggregatorCommand>();

        Self {
            subsystem_to_dispatcher_tx,
            subsystem_to_dispatcher_rx,
            dispatcher_to_engine_tx,
            dispatcher_to_engine_rx,
            dispatcher_to_submission_tx,
            dispatcher_to_submission_rx,
            dispatcher_to_aggregator_tx,
            dispatcher_to_aggregator_rx,
        }
    }
}
