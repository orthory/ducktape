use super::*;
use serde_json::{Value, json};
use std::fs;
use ui_lang_template::trace::{
    ARTIFACT_KIND, Action as RecordedAction, Artifact, Configuration, Environment, Finding,
    FindingKind, GENERATOR_VERSION, Mode, Phase, Reduction, ReductionAttempt, SCHEMA_VERSION,
    Sample, SourceLocation, Summary, WorstState,
};

pub(super) struct Campaign {
    pub(super) mode: Mode,
    pub(super) seed: Option<u64>,
    pub(super) steps: Option<usize>,
    pub(super) confirmations: usize,
    pub(super) deadline_ms: Option<f64>,
    pub(super) max_to_median_ratio: Option<f64>,
    pub(super) replay: Option<Artifact>,
}

impl Campaign {
    pub(super) fn from_env() -> Option<Self> {
        let mode = std::env::var("ICE_TRACE_MODE").ok()?;
        let confirmations = parsed_env::<usize>("ICE_TRACE_CONFIRMATIONS").unwrap_or(2);
        assert!(
            confirmations > 0,
            "ICE_TRACE_CONFIRMATIONS must be positive"
        );
        let deadline_ms = parsed_env("ICE_TRACE_DEADLINE_MS");
        let max_to_median_ratio = parsed_env("ICE_TRACE_MAX_TO_MEDIAN");
        match mode.as_str() {
            "fuzz" => Some(Self {
                mode: Mode::Fuzz,
                seed: Some(
                    parsed_env("ICE_TRACE_SEED")
                        .expect("ICE_TRACE_SEED is required for a fuzz campaign"),
                ),
                steps: Some(
                    parsed_env("ICE_TRACE_STEPS")
                        .expect("ICE_TRACE_STEPS is required for a fuzz campaign"),
                ),
                confirmations,
                deadline_ms,
                max_to_median_ratio,
                replay: None,
            }),
            "replay" => {
                let path = std::env::var_os("ICE_TRACE_REPLAY")
                    .map(PathBuf::from)
                    .expect("ICE_TRACE_REPLAY is required for replay");
                let bytes = fs::read(&path).unwrap_or_else(|error| {
                    panic!("cannot read replay artifact {}: {error}", path.display())
                });
                let artifact: Artifact = serde_json::from_slice(&bytes).unwrap_or_else(|error| {
                    panic!("invalid replay artifact {}: {error}", path.display())
                });
                artifact.validate().unwrap_or_else(|error| {
                    panic!("invalid replay artifact {}: {error}", path.display())
                });
                Some(Self {
                    mode: Mode::Replay,
                    seed: artifact.seed,
                    steps: Some(artifact.actions.len()),
                    confirmations,
                    deadline_ms: deadline_ms.or(artifact.configuration.deadline_ms),
                    max_to_median_ratio: max_to_median_ratio
                        .or(artifact.configuration.max_to_median_ratio),
                    replay: Some(artifact),
                })
            }
            _ => None,
        }
    }

    fn configuration(&self) -> Configuration {
        Configuration {
            mode: self.mode,
            test: None,
            warmup: 0,
            repeat: 1,
            steps: self.steps,
            confirmations: self.confirmations,
            deadline_ms: self.deadline_ms,
            max_to_median_ratio: self.max_to_median_ratio,
            generator_version: (self.mode == Mode::Fuzz).then_some(GENERATOR_VERSION),
        }
    }
}

pub(super) fn run_campaign<P>(
    program: impl Fn() -> P,
    config: Config,
    campaign: Campaign,
) -> Artifact
where
    P: Program + 'static,
    P::Renderer: 'static,
    P::Message: Clone,
{
    let configuration = campaign.configuration();
    let seed = campaign.seed;
    let mut artifact = if let Some(replay) = &campaign.replay {
        execute_recorded(&program, &config, &replay.actions, &configuration, seed)
    } else {
        execute_generated(
            &program,
            &config,
            seed.expect("fuzz seed is present"),
            campaign.steps.expect("fuzz steps are present"),
            &configuration,
        )
    };
    let candidate = artifact
        .finding
        .clone()
        .or_else(|| latency_finding(&artifact, &configuration));
    artifact.finding = None;

    if let Some(mut finding) = candidate {
        let mut confirmed = 1;
        while confirmed < campaign.confirmations {
            let rerun =
                execute_recorded(&program, &config, &artifact.actions, &configuration, seed);
            let rerun_finding = rerun
                .finding
                .clone()
                .or_else(|| latency_finding(&rerun, &configuration));
            if rerun_finding
                .as_ref()
                .is_none_or(|rerun| rerun.fingerprint != finding.fingerprint)
            {
                break;
            }
            confirmed += 1;
        }
        if confirmed == campaign.confirmations {
            finding.confirmed_runs = confirmed;
            artifact.finding = Some(finding.clone());
            if artifact.actions.len() > 1 {
                artifact.reduction = reduce(
                    &program,
                    &config,
                    &artifact.actions,
                    &configuration,
                    seed,
                    &finding.fingerprint,
                );
            }
            let actions = artifact
                .reduction
                .as_ref()
                .map_or(&artifact.actions, |reduction| &reduction.minimized_actions);
            let original_action = &artifact.actions[finding.action_index];
            let reduced_index = actions
                .iter()
                .rposition(|action| same_semantic_action(action, original_action))
                .unwrap_or_else(|| finding.action_index.min(actions.len().saturating_sub(1)));
            let duration_ns = artifact
                .summaries
                .iter()
                .find(|summary| {
                    summary.action_index == finding.action_index
                        && Some(summary.phase) == finding.phase
                })
                .map_or(0, |summary| summary.max_ns);
            if let Some(mut worst) = capture_worst(
                &program,
                &config,
                actions,
                reduced_index,
                finding.phase.unwrap_or(Phase::Action),
                duration_ns,
            ) {
                worst.action_index = finding.action_index;
                artifact.worst_states.push(worst);
            }
        }
    }
    artifact
}

fn execute_generated<P>(
    program: &impl Fn() -> P,
    config: &Config,
    seed: u64,
    steps: usize,
    configuration: &Configuration,
) -> Artifact
where
    P: Program + 'static,
    P::Renderer: 'static,
    P::Message: Clone,
{
    let mut driver = Driver::new(program(), config.clone());
    driver.enable_trace(config, configuration.clone(), Some(seed));
    let mut generator = Generator::new(seed);
    for index in 0..steps {
        let inventory = driver.interaction_inventory();
        let action = generator.next(&inventory, driver.viewport());
        let source = Location::new(
            config.source.map_or("<fuzz>", |source| source.path),
            config.source.map_or(1, |source| source.line),
            config.source.map_or(1, |source| source.column),
            leak(format!("fuzz action {index}")),
        );
        let result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
            driver.perform_action(action, source)
        }));
        if let Err(payload) = result {
            driver.finish_trace_action();
            let message = panic_payload(&payload);
            let action_index = driver
                .trace
                .as_ref()
                .map_or(0, |trace| trace.artifact().actions.len().saturating_sub(1));
            let action_identity = driver
                .trace
                .as_ref()
                .and_then(|trace| trace.artifact().actions.get(action_index))
                .map(action_identity)
                .unwrap_or_else(|| "unknown".into());
            let kind = classify_failure(&message);
            let fingerprint = fingerprint(kind, &action_identity, None, &message);
            if let Some(trace) = &mut driver.trace {
                trace.artifact_mut().finding = Some(Finding {
                    kind,
                    fingerprint,
                    action_index,
                    phase: None,
                    message,
                    confirmed_runs: 1,
                });
            }
            break;
        }
    }
    driver.take_trace()
}

fn execute_recorded<P>(
    program: &impl Fn() -> P,
    config: &Config,
    actions: &[RecordedAction],
    configuration: &Configuration,
    seed: Option<u64>,
) -> Artifact
where
    P: Program + 'static,
    P::Renderer: 'static,
    P::Message: Clone,
{
    let mut driver = Driver::new(program(), config.clone());
    driver.enable_trace(config, configuration.clone(), seed);
    for recorded in actions {
        let action = replay_action(recorded).unwrap_or_else(|error| panic!("{error}"));
        let source = replay_location(&recorded.source);
        let result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
            driver.perform_action(action, source)
        }));
        if let Err(payload) = result {
            driver.finish_trace_action();
            let message = panic_payload(&payload);
            let kind = classify_failure(&message);
            let fingerprint = fingerprint(kind, &action_identity(recorded), None, &message);
            if let Some(trace) = &mut driver.trace {
                trace.artifact_mut().finding = Some(Finding {
                    kind,
                    fingerprint,
                    action_index: recorded.index,
                    phase: None,
                    message,
                    confirmed_runs: 1,
                });
            }
            break;
        }
    }
    driver.take_trace()
}

fn latency_finding(artifact: &Artifact, configuration: &Configuration) -> Option<Finding> {
    let mut totals = vec![0_u64; artifact.actions.len()];
    for sample in &artifact.samples {
        if sample.phase == Phase::Action {
            totals[sample.action_index] = sample.duration_ns;
        }
    }
    let mut by_kind = std::collections::BTreeMap::<&str, Vec<u64>>::new();
    for (action, duration) in artifact.actions.iter().zip(&totals) {
        by_kind.entry(&action.kind).or_default().push(*duration);
    }
    for values in by_kind.values_mut() {
        values.sort_unstable();
    }
    let deadline_ns = configuration
        .deadline_ms
        .map(|milliseconds| (milliseconds * 1_000_000.0) as u64);
    artifact
        .actions
        .iter()
        .enumerate()
        .filter_map(|(index, action)| {
            let duration = totals[index];
            let deadline = deadline_ns.is_some_and(|deadline| duration > deadline);
            let ratio = configuration.max_to_median_ratio.is_some_and(|ratio| {
                let values = &by_kind[action.kind.as_str()];
                values.len() >= 3 && duration as f64 > percentile(values, 50) as f64 * ratio
            });
            (deadline || ratio).then_some((index, action, duration))
        })
        .max_by_key(|(_, _, duration)| *duration)
        .map(|(action_index, action, duration)| Finding {
            kind: FindingKind::Latency,
            fingerprint: fingerprint(
                FindingKind::Latency,
                &action_identity(action),
                Some(Phase::Action),
                "",
            ),
            action_index,
            phase: Some(Phase::Action),
            message: format!("confirmed action latency outlier: {duration}ns"),
            confirmed_runs: 1,
        })
}

fn reduce<P>(
    program: &impl Fn() -> P,
    config: &Config,
    actions: &[RecordedAction],
    configuration: &Configuration,
    seed: Option<u64>,
    fingerprint: &str,
) -> Option<Reduction>
where
    P: Program + 'static,
    P::Renderer: 'static,
    P::Message: Clone,
{
    let mut current = actions.to_vec();
    let mut attempts = Vec::new();
    let mut granularity = 2;
    while current.len() >= 2 {
        let chunk = current.len().div_ceil(granularity);
        let mut reduced = false;
        for start in (0..current.len()).step_by(chunk) {
            let end = (start + chunk).min(current.len());
            let candidate = reindex(
                current
                    .iter()
                    .enumerate()
                    .filter(|(index, _)| *index < start || *index >= end)
                    .map(|(_, action)| action.clone())
                    .collect(),
            );
            if candidate.is_empty() {
                continue;
            }
            let replay = execute_recorded(program, config, &candidate, configuration, seed);
            let replay_finding = replay
                .finding
                .clone()
                .or_else(|| latency_finding(&replay, configuration));
            let preserved = replay_finding
                .as_ref()
                .is_some_and(|finding| finding.fingerprint == fingerprint);
            attempts.push(ReductionAttempt {
                candidate_len: candidate.len(),
                preserved,
            });
            if preserved {
                current = candidate;
                granularity = granularity.saturating_sub(1).max(2);
                reduced = true;
                break;
            }
        }
        if !reduced {
            if granularity >= current.len() {
                break;
            }
            granularity = (granularity * 2).min(current.len());
        }
    }
    (current.len() < actions.len()).then_some(Reduction {
        original_len: actions.len(),
        minimized_actions: current,
        attempts,
    })
}

fn reindex(mut actions: Vec<RecordedAction>) -> Vec<RecordedAction> {
    for (index, action) in actions.iter_mut().enumerate() {
        action.index = index;
    }
    actions
}

fn capture_worst<P>(
    program: &impl Fn() -> P,
    config: &Config,
    actions: &[RecordedAction],
    action_index: usize,
    phase: Phase,
    duration_ns: u64,
) -> Option<WorstState>
where
    P: Program + 'static,
    P::Renderer: 'static,
    P::Message: Clone,
{
    let mut driver = Driver::new(program(), config.clone());
    for (replay_index, recorded) in actions.iter().take(action_index + 1).enumerate() {
        let action = replay_action(recorded).ok()?;
        if std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
            driver.perform_action(action, replay_location(&recorded.source))
        }))
        .is_err()
        {
            if replay_index != action_index {
                return None;
            }
            break;
        }
    }
    let capture = driver.capture(
        &format!("trace_worst_{action_index}"),
        replay_location(&actions[action_index].source),
    );
    Some(WorstState {
        action_index,
        phase,
        duration_ns,
        png: capture.png_path.to_string_lossy().into_owned(),
        manifest: capture.metadata_path.to_string_lossy().into_owned(),
    })
}

fn classify_failure(message: &str) -> FindingKind {
    if message.contains("quiescence within") {
        FindingKind::Timeout
    } else if message.contains("expectation failed") {
        FindingKind::Assertion
    } else {
        FindingKind::Panic
    }
}

fn panic_payload(payload: &Box<dyn Any + Send>) -> String {
    payload
        .downcast_ref::<String>()
        .cloned()
        .or_else(|| {
            payload
                .downcast_ref::<&'static str>()
                .map(|value| (*value).into())
        })
        .unwrap_or_else(|| "non-string panic payload".into())
}

fn fingerprint(kind: FindingKind, action: &str, phase: Option<Phase>, _message: &str) -> String {
    let mut hash = 0xcbf2_9ce4_8422_2325_u64;
    for byte in format!("{kind:?}\0{action}\0{phase:?}").bytes() {
        hash ^= u64::from(byte);
        hash = hash.wrapping_mul(0x0000_0100_0000_01b3);
    }
    format!("{hash:016x}")
}

fn action_identity(action: &RecordedAction) -> String {
    action.target.as_ref().map_or_else(
        || action.kind.clone(),
        |target| format!("{}@{target}", action.kind),
    )
}

fn same_semantic_action(left: &RecordedAction, right: &RecordedAction) -> bool {
    left.kind == right.kind && left.target == right.target && left.parameters == right.parameters
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub(super) struct InteractionTarget {
    pub(super) id: String,
    pub(super) visible: bool,
    pub(super) scrollable: bool,
    pub(super) focusable: bool,
}

struct Generator {
    random: SplitMix64,
    cursor_inside: bool,
}

impl Generator {
    fn new(seed: u64) -> Self {
        Self {
            random: SplitMix64(seed),
            cursor_inside: false,
        }
    }

    fn next(&mut self, inventory: &[InteractionTarget], viewport: Size) -> Action {
        let visible = inventory
            .iter()
            .filter(|target| target.visible)
            .collect::<Vec<_>>();
        let focusable = visible
            .iter()
            .copied()
            .filter(|target| target.focusable)
            .collect::<Vec<_>>();
        let scrollable = visible
            .iter()
            .copied()
            .filter(|target| target.scrollable)
            .collect::<Vec<_>>();
        let mut policies = vec!["redraw", "advance", "resize", "key", "chord"];
        if !visible.is_empty() {
            policies.extend(["move_to", "click"]);
        }
        if !focusable.is_empty() {
            policies.push("focus");
        }
        if !scrollable.is_empty() {
            policies.push("scroll");
        }
        if self.cursor_inside {
            policies.extend(["wheel", "leave"]);
        }
        match policies[self.random.index(policies.len())] {
            "move_to" => {
                self.cursor_inside = true;
                Action::MoveTo(visible[self.random.index(visible.len())].id.clone())
            }
            "click" => {
                self.cursor_inside = true;
                Action::Click {
                    target: visible[self.random.index(visible.len())].id.clone(),
                    button: MouseButton::Left,
                    count: 1,
                }
            }
            "focus" => Action::Focus(focusable[self.random.index(focusable.len())].id.clone()),
            "scroll" => Action::ScrollBy {
                target: scrollable[self.random.index(scrollable.len())].id.clone(),
                x: 0.0,
                y: if self.random.boolean() { 48.0 } else { -48.0 },
            },
            "wheel" => Action::Wheel(WheelDelta::Pixels {
                x: 0.0,
                y: if self.random.boolean() { 48.0 } else { -48.0 },
            }),
            "leave" => {
                self.cursor_inside = false;
                Action::Leave
            }
            "resize" => {
                let width =
                    (viewport.width + if self.random.boolean() { 32.0 } else { -32.0 }).max(160.0);
                let height =
                    (viewport.height + if self.random.boolean() { 24.0 } else { -24.0 }).max(120.0);
                Action::Resize(Size::new(width, height))
            }
            "key" => Action::Key(match self.random.index(3) {
                0 => Key::named(keyboard::key::Named::Enter),
                1 => Key::named(keyboard::key::Named::Escape),
                _ => Key::named(keyboard::key::Named::ArrowDown),
            }),
            "chord" => Action::Chord {
                modifiers: Modifiers::new(false, true, false, false),
                key: Key::character("a"),
            },
            "advance" => Action::Advance(Duration::from_millis(match self.random.index(3) {
                0 => 1,
                1 => 16,
                _ => 100,
            })),
            _ => Action::Redraw,
        }
    }
}

struct SplitMix64(u64);

impl SplitMix64 {
    fn next(&mut self) -> u64 {
        self.0 = self.0.wrapping_add(0x9e37_79b9_7f4a_7c15);
        let mut value = self.0;
        value = (value ^ (value >> 30)).wrapping_mul(0xbf58_476d_1ce4_e5b9);
        value = (value ^ (value >> 27)).wrapping_mul(0x94d0_49bb_1331_11eb);
        value ^ (value >> 31)
    }

    fn index(&mut self, length: usize) -> usize {
        usize::try_from(self.next() % length as u64).expect("bounded random index fits usize")
    }

    fn boolean(&mut self) -> bool {
        self.next() & 1 == 1
    }
}

struct CurrentAction {
    started: Instant,
    index: usize,
}

pub(super) struct Recorder {
    artifact: Artifact,
    output: Option<PathBuf>,
    current: Option<CurrentAction>,
}

impl Recorder {
    pub(super) fn from_env<P>(driver: &Driver<P>, config: &Config) -> Option<Self>
    where
        P: Program + 'static,
        P::Renderer: 'static,
        P::Message: Clone,
    {
        if std::env::var("ICE_TRACE_MODE").ok().as_deref() != Some("authored") {
            return None;
        }
        let output = std::env::var_os("ICE_TRACE_RESULT").map(PathBuf::from)?;
        Some(Self::new(
            environment(driver, config),
            Configuration {
                mode: Mode::Authored,
                test: Some(config.name.to_owned()),
                warmup: 0,
                repeat: 1,
                steps: None,
                confirmations: parsed_env::<usize>("ICE_TRACE_CONFIRMATIONS").unwrap_or(1),
                deadline_ms: parsed_env("ICE_TRACE_DEADLINE_MS"),
                max_to_median_ratio: parsed_env("ICE_TRACE_MAX_TO_MEDIAN"),
                generator_version: None,
            },
            None,
            Some(output),
        ))
    }

    pub(super) fn new(
        environment: Environment,
        configuration: Configuration,
        seed: Option<u64>,
        output: Option<PathBuf>,
    ) -> Self {
        Self {
            artifact: Artifact {
                artifact_kind: ARTIFACT_KIND.into(),
                schema_version: SCHEMA_VERSION,
                app_root: std::env::var("ICE_TRACE_APP_ROOT")
                    .map(|path| manifest_source_path(&path))
                    .unwrap_or_else(|_| "<generated-app>".into()),
                package: std::env::var("ICE_TRACE_PACKAGE")
                    .unwrap_or_else(|_| "<generated-package>".into()),
                environment,
                configuration,
                seed,
                actions: Vec::new(),
                samples: Vec::new(),
                summaries: Vec::new(),
                unavailable_phases: vec![Phase::Draw],
                finding: None,
                worst_states: Vec::new(),
                reduction: None,
            },
            output,
            current: None,
        }
    }

    pub(super) fn for_campaign<P>(
        driver: &Driver<P>,
        config: &Config,
        configuration: Configuration,
        seed: Option<u64>,
    ) -> Self
    where
        P: Program + 'static,
        P::Renderer: 'static,
        P::Message: Clone,
    {
        Self::new(environment(driver, config), configuration, seed, None)
    }

    pub(super) fn begin(
        &mut self,
        action: &Action,
        source: Location,
        target_source: Option<Location>,
    ) -> usize {
        let index = self.artifact.actions.len();
        self.artifact
            .actions
            .push(record_action(index, action, source, target_source));
        self.current = Some(CurrentAction {
            started: Instant::now(),
            index,
        });
        index
    }

    pub(super) fn record_untimed(
        &mut self,
        action: &Action,
        source: Location,
        target_source: Option<Location>,
    ) -> usize {
        let index = self.artifact.actions.len();
        self.artifact
            .actions
            .push(record_action(index, action, source, target_source));
        index
    }

    pub(super) fn phase(&mut self, phase: Phase, duration: Duration) {
        let Some(current) = &self.current else {
            return;
        };
        self.artifact.samples.push(Sample {
            run: 0,
            action_index: current.index,
            phase,
            duration_ns: duration_ns(duration),
        });
    }

    pub(super) fn finish(&mut self) {
        let Some(current) = self.current.take() else {
            return;
        };
        self.artifact.samples.push(Sample {
            run: 0,
            action_index: current.index,
            phase: Phase::Action,
            duration_ns: duration_ns(current.started.elapsed()),
        });
    }

    pub(super) fn is_recording_action(&self) -> bool {
        self.current.is_some()
    }

    pub(super) fn artifact(&self) -> &Artifact {
        &self.artifact
    }

    pub(super) fn artifact_mut(&mut self) -> &mut Artifact {
        &mut self.artifact
    }

    pub(super) fn into_artifact(mut self) -> Artifact {
        self.finish();
        self.artifact.summaries = summaries(&self.artifact.samples);
        self.output = None;
        self.artifact.clone()
    }

    fn write(&mut self) {
        self.finish();
        self.artifact.summaries = summaries(&self.artifact.samples);
        let Some(path) = self.output.take() else {
            return;
        };
        if let Some(parent) = path.parent() {
            let _ = fs::create_dir_all(parent);
        }
        if self.artifact.validate().is_err() {
            return;
        }
        let Ok(mut bytes) = serde_json::to_vec_pretty(&self.artifact) else {
            return;
        };
        bytes.push(b'\n');
        let _ = fs::write(path, bytes);
    }
}

impl Drop for Recorder {
    fn drop(&mut self) {
        self.write();
    }
}

pub(super) fn source_location(source: Location) -> SourceLocation {
    SourceLocation {
        path: manifest_source_path(source.path),
        line: source.line,
        column: source.column,
        statement: source.statement.to_owned(),
    }
}

pub(super) fn primary_target(action: &Action) -> Option<&str> {
    match action {
        Action::MoveTo(target)
        | Action::SnapEnd(target)
        | Action::DropAt(target)
        | Action::Focus(target) => Some(target),
        Action::Click { target, .. }
        | Action::Press { target, .. }
        | Action::ScrollTo { target, .. }
        | Action::ScrollBy { target, .. }
        | Action::Snap { target, .. }
        | Action::Tap { target, .. }
        | Action::Accessibility { target, .. } => Some(target),
        Action::Drag { from, .. } => Some(from),
        _ => None,
    }
}

fn replay_action(action: &RecordedAction) -> Result<Action, String> {
    let target = || {
        action.target.clone().ok_or_else(|| {
            format!(
                "replay action {} `{}` omitted its target",
                action.index, action.kind
            )
        })
    };
    let number = |name: &str| {
        action.parameters[name]
            .as_f64()
            .map(|value| value as f32)
            .ok_or_else(|| {
                format!(
                    "replay action {} `{}` omitted numeric parameter `{name}`",
                    action.index, action.kind
                )
            })
    };
    let unsigned = |name: &str| {
        action.parameters[name].as_u64().ok_or_else(|| {
            format!(
                "replay action {} `{}` omitted unsigned parameter `{name}`",
                action.index, action.kind
            )
        })
    };
    match action.kind.as_str() {
        "leave" => Ok(Action::Leave),
        "move_to" => Ok(Action::MoveTo(target()?)),
        "move_to_point" => Ok(Action::MoveToPoint(Point::new(number("x")?, number("y")?))),
        "click" => Ok(Action::Click {
            target: target()?,
            button: decode_mouse_button(&action.parameters["button"])?,
            count: u8::try_from(unsigned("count")?)
                .map_err(|_| format!("replay action {} click count exceeds u8", action.index))?,
        }),
        "click_at" => Ok(Action::ClickAt {
            position: Point::new(number("x")?, number("y")?),
            button: decode_mouse_button(&action.parameters["button"])?,
            count: u8::try_from(unsigned("count")?)
                .map_err(|_| format!("replay action {} click count exceeds u8", action.index))?,
        }),
        "press" => Ok(Action::Press {
            target: target()?,
            button: decode_mouse_button(&action.parameters["button"])?,
        }),
        "release" => Ok(Action::Release(decode_mouse_button(
            &action.parameters["button"],
        )?)),
        "wheel" => {
            let x = number("x")?;
            let y = number("y")?;
            match action.parameters["unit"].as_str() {
                Some("lines") => Ok(Action::Wheel(WheelDelta::Lines { x, y })),
                Some("pixels") => Ok(Action::Wheel(WheelDelta::Pixels { x, y })),
                _ => Err(format!(
                    "replay action {} wheel has unsupported unit",
                    action.index
                )),
            }
        }
        "scroll_to" => Ok(Action::ScrollTo {
            target: target()?,
            x: number("x")?,
            y: number("y")?,
        }),
        "scroll_by" => Ok(Action::ScrollBy {
            target: target()?,
            x: number("x")?,
            y: number("y")?,
        }),
        "snap" => Ok(Action::Snap {
            target: target()?,
            x: number("x")?,
            y: number("y")?,
        }),
        "snap_end" => Ok(Action::SnapEnd(target()?)),
        "drag" => Ok(Action::Drag {
            from: target()?,
            to: string_parameter(action, "to")?,
        }),
        "drop_at" => Ok(Action::DropAt(target()?)),
        "focus" => Ok(Action::Focus(target()?)),
        "focus_next" => Ok(Action::FocusNext),
        "focus_previous" => Ok(Action::FocusPrevious),
        "blur" => Ok(Action::Blur),
        "window_focus" => Ok(Action::WindowFocus(
            action.parameters["focused"]
                .as_bool()
                .ok_or_else(|| format!("replay action {} omitted `focused`", action.index))?,
        )),
        "type" => Ok(Action::Type(string_parameter(action, "value")?)),
        "clear" => Ok(Action::Clear),
        "replace" => Ok(Action::Replace(string_parameter(action, "value")?)),
        "select" => Ok(Action::Select {
            start: usize::try_from(unsigned("start")?)
                .map_err(|_| format!("replay action {} selection start overflows", action.index))?,
            end: usize::try_from(unsigned("end")?)
                .map_err(|_| format!("replay action {} selection end overflows", action.index))?,
        }),
        "select_all" => Ok(Action::SelectAll),
        "cursor" => Ok(Action::Cursor(
            usize::try_from(unsigned("position")?)
                .map_err(|_| format!("replay action {} cursor overflows", action.index))?,
        )),
        "cursor_front" => Ok(Action::CursorFront),
        "cursor_end" => Ok(Action::CursorEnd),
        "composition" => Ok(Action::Composition(decode_composition(action)?)),
        "key" => Ok(Action::Key(decode_key(&action.parameters["key"])?)),
        "key_down" => Ok(Action::KeyDown {
            key: decode_key(&action.parameters["key"])?,
            metadata: decode_key_metadata(action)?,
        }),
        "key_up" => Ok(Action::KeyUp {
            key: decode_key(&action.parameters["key"])?,
            metadata: decode_key_metadata(action)?,
        }),
        "modifiers" => Ok(Action::Modifiers(decode_modifiers(&action.parameters)?)),
        "chord" => Ok(Action::Chord {
            modifiers: decode_modifiers(&action.parameters["modifiers"])?,
            key: decode_key(&action.parameters["key"])?,
        }),
        "repeat" => Ok(Action::Repeat {
            key: decode_key(&action.parameters["key"])?,
            count: usize::try_from(unsigned("count")?)
                .map_err(|_| format!("replay action {} repeat count overflows", action.index))?,
        }),
        "touch" => Ok(Action::Touch {
            phase: match action.parameters["phase"].as_str() {
                Some("down") => TouchPhase::Down,
                Some("move") => TouchPhase::Move,
                Some("up") => TouchPhase::Up,
                Some("cancel") => TouchPhase::Cancel,
                _ => {
                    return Err(format!(
                        "replay action {} has unsupported touch phase",
                        action.index
                    ));
                }
            },
            id: unsigned("id")?,
            position: Point::new(number("x")?, number("y")?),
        }),
        "tap" => Ok(Action::Tap {
            target: target()?,
            count: u8::try_from(unsigned("count")?)
                .map_err(|_| format!("replay action {} tap count exceeds u8", action.index))?,
        }),
        "window_opened" => Ok(Action::WindowOpened),
        "window_closed" => Ok(Action::WindowClosed),
        "window_move" => Ok(Action::WindowMove(Point::new(number("x")?, number("y")?))),
        "resize" => Ok(Action::Resize(Size::new(
            number("width")?,
            number("height")?,
        ))),
        "rescale" => Ok(Action::Rescale(number("scale")?)),
        "close_requested" => Ok(Action::CloseRequested),
        "redraw" => Ok(Action::Redraw),
        "system_theme" => Ok(Action::SystemTheme(
            match action.parameters["theme"].as_str() {
                Some("none") => ThemeMode::None,
                Some("light") => ThemeMode::Light,
                Some("dark") => ThemeMode::Dark,
                _ => {
                    return Err(format!(
                        "replay action {} has unsupported system theme",
                        action.index
                    ));
                }
            },
        )),
        "file_hover" => Ok(Action::FileHover(PathBuf::from(string_parameter(
            action, "path",
        )?))),
        "file_drop" => Ok(Action::FileDrop(PathBuf::from(string_parameter(
            action, "path",
        )?))),
        "file_leave" => Ok(Action::FileLeave),
        "wait" => Ok(Action::Wait(Duration::from_millis(unsigned(
            "duration_ms",
        )?))),
        "advance" => Ok(Action::Advance(Duration::from_millis(unsigned(
            "duration_ms",
        )?))),
        "idle" => Ok(Action::Idle),
        "capture" => Ok(Action::Capture(string_parameter(action, "name")?)),
        "accessibility" => Ok(Action::Accessibility {
            action: match action.parameters["action"].as_str() {
                Some("click") => AccessibilityAction::Click,
                Some("focus") => AccessibilityAction::Focus,
                _ => {
                    return Err(format!(
                        "replay action {} has unsupported accessibility action",
                        action.index
                    ));
                }
            },
            target: target()?,
        }),
        unsupported => Err(format!(
            "replay action {} uses unsupported recorded action `{unsupported}`",
            action.index
        )),
    }
}

fn replay_location(source: &SourceLocation) -> Location {
    Location::new(
        leak(source.path.clone()),
        source.line,
        source.column,
        leak(source.statement.clone()),
    )
}

fn leak(value: String) -> &'static str {
    Box::leak(value.into_boxed_str())
}

fn string_parameter(action: &RecordedAction, name: &str) -> Result<String, String> {
    action.parameters[name]
        .as_str()
        .map(str::to_owned)
        .ok_or_else(|| {
            format!(
                "replay action {} `{}` omitted string parameter `{name}`",
                action.index, action.kind
            )
        })
}

fn decode_mouse_button(value: &Value) -> Result<MouseButton, String> {
    match value.as_str() {
        Some("left") => Ok(MouseButton::Left),
        Some("right") => Ok(MouseButton::Right),
        Some("middle") => Ok(MouseButton::Middle),
        Some("back") => Ok(MouseButton::Back),
        Some("forward") => Ok(MouseButton::Forward),
        _ => value["other"]
            .as_u64()
            .and_then(|value| u16::try_from(value).ok())
            .map(MouseButton::Other)
            .ok_or_else(|| "unsupported replay mouse button".into()),
    }
}

fn decode_key(value: &Value) -> Result<Key, String> {
    if let Some(value) = value["character"].as_str() {
        return Ok(Key::character(value));
    }
    if value["unidentified"] == true {
        return Ok(Key::Unidentified);
    }
    let named = value["named"]
        .as_str()
        .ok_or_else(|| "replay key omitted `named` or `character`".to_owned())?;
    let named = match named {
        "Enter" => keyboard::key::Named::Enter,
        "Escape" => keyboard::key::Named::Escape,
        "ArrowDown" => keyboard::key::Named::ArrowDown,
        "ArrowUp" => keyboard::key::Named::ArrowUp,
        "ArrowLeft" => keyboard::key::Named::ArrowLeft,
        "ArrowRight" => keyboard::key::Named::ArrowRight,
        "Tab" => keyboard::key::Named::Tab,
        "Home" => keyboard::key::Named::Home,
        "End" => keyboard::key::Named::End,
        "PageUp" => keyboard::key::Named::PageUp,
        "PageDown" => keyboard::key::Named::PageDown,
        "Space" => keyboard::key::Named::Space,
        "Backspace" => keyboard::key::Named::Backspace,
        "Delete" => keyboard::key::Named::Delete,
        _ => return Err(format!("unsupported replay named key {named:?}")),
    };
    Ok(Key::named(named))
}

fn decode_modifiers(value: &Value) -> Result<Modifiers, String> {
    let field = |name| {
        value[name]
            .as_bool()
            .ok_or_else(|| format!("replay modifiers omitted `{name}`"))
    };
    Ok(Modifiers::new(
        field("shift")?,
        field("control")?,
        field("alt")?,
        field("logo")?,
    ))
}

fn decode_composition(action: &RecordedAction) -> Result<CompositionPhase, String> {
    match action.parameters["phase"].as_str() {
        Some("start") => Ok(CompositionPhase::Start),
        Some("update") => {
            let selection = action.parameters["selection"]
                .as_array()
                .map(|selection| {
                    let [start, end] = selection.as_slice() else {
                        return Err(format!(
                            "replay action {} composition selection requires two offsets",
                            action.index
                        ));
                    };
                    Ok(usize::try_from(start.as_u64().ok_or_else(|| {
                        format!("replay action {} has invalid selection start", action.index)
                    })?)
                    .map_err(|_| {
                        format!("replay action {} selection start overflows", action.index)
                    })?..usize::try_from(end.as_u64().ok_or_else(|| {
                        format!("replay action {} has invalid selection end", action.index)
                    })?)
                    .map_err(|_| {
                        format!("replay action {} selection end overflows", action.index)
                    })?)
                })
                .transpose()?;
            Ok(CompositionPhase::Update {
                text: string_parameter(action, "text")?,
                selection,
            })
        }
        Some("commit") => Ok(CompositionPhase::Commit(string_parameter(action, "text")?)),
        Some("cancel") => Ok(CompositionPhase::Cancel),
        _ => Err(format!(
            "replay action {} has unsupported composition phase",
            action.index
        )),
    }
}

fn decode_key_metadata(action: &RecordedAction) -> Result<KeyMetadata, String> {
    let metadata = &action.parameters["metadata"];
    let location = match metadata["location"].as_str() {
        Some("standard") => KeyLocation::Standard,
        Some("left") => KeyLocation::Left,
        Some("right") => KeyLocation::Right,
        Some("numpad") => KeyLocation::Numpad,
        _ => {
            return Err(format!(
                "replay action {} has invalid key location",
                action.index
            ));
        }
    };
    Ok(KeyMetadata {
        modified_key: (!metadata["modified_key"].is_null())
            .then(|| decode_key(&metadata["modified_key"]))
            .transpose()?,
        physical_key: decode_physical_key(&metadata["physical_key"]).map_err(|error| {
            format!(
                "replay action {} has invalid physical key: {error}",
                action.index
            )
        })?,
        location,
        text: metadata["text"].as_str().map(str::to_owned),
        repeat: metadata["repeat"]
            .as_bool()
            .ok_or_else(|| format!("replay action {} omitted key repeat metadata", action.index))?,
    })
}

fn decode_physical_key(value: &Value) -> Result<Option<keyboard::key::Physical>, String> {
    use keyboard::key::{Code, NativeCode, Physical};
    if value.is_null() {
        return Ok(None);
    }
    if let Some(name) = value["code"].as_str() {
        macro_rules! codes {
            ($($variant:ident),+ $(,)?) => {
                return Ok(Some(Physical::Code(match name {
                    $(stringify!($variant) => Code::$variant,)+
                    _ => return Err(format!("unsupported code {name:?}")),
                })))
            };
        }
        codes!(
            Backquote,
            Backslash,
            BracketLeft,
            BracketRight,
            Comma,
            Digit0,
            Digit1,
            Digit2,
            Digit3,
            Digit4,
            Digit5,
            Digit6,
            Digit7,
            Digit8,
            Digit9,
            Equal,
            IntlBackslash,
            IntlRo,
            IntlYen,
            KeyA,
            KeyB,
            KeyC,
            KeyD,
            KeyE,
            KeyF,
            KeyG,
            KeyH,
            KeyI,
            KeyJ,
            KeyK,
            KeyL,
            KeyM,
            KeyN,
            KeyO,
            KeyP,
            KeyQ,
            KeyR,
            KeyS,
            KeyT,
            KeyU,
            KeyV,
            KeyW,
            KeyX,
            KeyY,
            KeyZ,
            Minus,
            Period,
            Quote,
            Semicolon,
            Slash,
            AltLeft,
            AltRight,
            Backspace,
            CapsLock,
            ContextMenu,
            ControlLeft,
            ControlRight,
            Enter,
            SuperLeft,
            SuperRight,
            ShiftLeft,
            ShiftRight,
            Space,
            Tab,
            Convert,
            KanaMode,
            Lang1,
            Lang2,
            Lang3,
            Lang4,
            Lang5,
            NonConvert,
            Delete,
            End,
            Help,
            Home,
            Insert,
            PageDown,
            PageUp,
            ArrowDown,
            ArrowLeft,
            ArrowRight,
            ArrowUp,
            NumLock,
            Numpad0,
            Numpad1,
            Numpad2,
            Numpad3,
            Numpad4,
            Numpad5,
            Numpad6,
            Numpad7,
            Numpad8,
            Numpad9,
            NumpadAdd,
            NumpadBackspace,
            NumpadClear,
            NumpadClearEntry,
            NumpadComma,
            NumpadDecimal,
            NumpadDivide,
            NumpadEnter,
            NumpadEqual,
            NumpadHash,
            NumpadMemoryAdd,
            NumpadMemoryClear,
            NumpadMemoryRecall,
            NumpadMemoryStore,
            NumpadMemorySubtract,
            NumpadMultiply,
            NumpadParenLeft,
            NumpadParenRight,
            NumpadStar,
            NumpadSubtract,
            Escape,
            Fn,
            FnLock,
            PrintScreen,
            ScrollLock,
            Pause,
            BrowserBack,
            BrowserFavorites,
            BrowserForward,
            BrowserHome,
            BrowserRefresh,
            BrowserSearch,
            BrowserStop,
            Eject,
            LaunchApp1,
            LaunchApp2,
            LaunchMail,
            MediaPlayPause,
            MediaSelect,
            MediaStop,
            MediaTrackNext,
            MediaTrackPrevious,
            Power,
            Sleep,
            AudioVolumeDown,
            AudioVolumeMute,
            AudioVolumeUp,
            WakeUp,
            Meta,
            Hyper,
            Turbo,
            Abort,
            Resume,
            Suspend,
            Again,
            Copy,
            Cut,
            Find,
            Open,
            Paste,
            Props,
            Select,
            Undo,
            Hiragana,
            Katakana,
            F1,
            F2,
            F3,
            F4,
            F5,
            F6,
            F7,
            F8,
            F9,
            F10,
            F11,
            F12,
            F13,
            F14,
            F15,
            F16,
            F17,
            F18,
            F19,
            F20,
            F21,
            F22,
            F23,
            F24,
            F25,
            F26,
            F27,
            F28,
            F29,
            F30,
            F31,
            F32,
            F33,
            F34,
            F35,
        );
    }
    let native = &value["native"];
    let code = || {
        native["code"]
            .as_u64()
            .ok_or_else(|| "native key omitted code".to_owned())
    };
    let native = match native["kind"].as_str() {
        Some("unidentified") => NativeCode::Unidentified,
        Some("android") => NativeCode::Android(
            u32::try_from(code()?).map_err(|_| "Android native code exceeds u32".to_owned())?,
        ),
        Some("macos") => NativeCode::MacOS(
            u16::try_from(code()?).map_err(|_| "macOS native code exceeds u16".to_owned())?,
        ),
        Some("windows") => NativeCode::Windows(
            u16::try_from(code()?).map_err(|_| "Windows native code exceeds u16".to_owned())?,
        ),
        Some("xkb") => NativeCode::Xkb(
            u32::try_from(code()?).map_err(|_| "XKB native code exceeds u32".to_owned())?,
        ),
        _ => return Err("physical key omitted `code` or supported `native`".into()),
    };
    Ok(Some(Physical::Unidentified(native)))
}

fn record_action(
    index: usize,
    action: &Action,
    source: Location,
    target_source: Option<Location>,
) -> RecordedAction {
    let (kind, target, parameters) = match action {
        Action::Leave => ("leave", None, Value::Null),
        Action::MoveTo(target) => ("move_to", Some(target.clone()), Value::Null),
        Action::MoveToPoint(point) => {
            ("move_to_point", None, json!({ "x": point.x, "y": point.y }))
        }
        Action::Click {
            target,
            button,
            count,
        } => (
            "click",
            Some(target.clone()),
            json!({ "button": mouse_button(*button), "count": count }),
        ),
        Action::ClickAt {
            position,
            button,
            count,
        } => (
            "click_at",
            None,
            json!({
                "x": position.x,
                "y": position.y,
                "button": mouse_button(*button),
                "count": count,
            }),
        ),
        Action::Press { target, button } => (
            "press",
            Some(target.clone()),
            json!({ "button": mouse_button(*button) }),
        ),
        Action::Release(button) => ("release", None, json!({ "button": mouse_button(*button) })),
        Action::Wheel(delta) => match delta {
            WheelDelta::Lines { x, y } => {
                ("wheel", None, json!({ "unit": "lines", "x": x, "y": y }))
            }
            WheelDelta::Pixels { x, y } => {
                ("wheel", None, json!({ "unit": "pixels", "x": x, "y": y }))
            }
        },
        Action::ScrollTo { target, x, y } => {
            ("scroll_to", Some(target.clone()), json!({ "x": x, "y": y }))
        }
        Action::ScrollBy { target, x, y } => {
            ("scroll_by", Some(target.clone()), json!({ "x": x, "y": y }))
        }
        Action::Snap { target, x, y } => ("snap", Some(target.clone()), json!({ "x": x, "y": y })),
        Action::SnapEnd(target) => ("snap_end", Some(target.clone()), Value::Null),
        Action::Drag { from, to } => ("drag", Some(from.clone()), json!({ "to": to })),
        Action::DropAt(target) => ("drop_at", Some(target.clone()), Value::Null),
        Action::Focus(target) => ("focus", Some(target.clone()), Value::Null),
        Action::FocusNext => ("focus_next", None, Value::Null),
        Action::FocusPrevious => ("focus_previous", None, Value::Null),
        Action::Blur => ("blur", None, Value::Null),
        Action::WindowFocus(focused) => ("window_focus", None, json!({ "focused": focused })),
        Action::Type(value) => ("type", None, json!({ "value": value })),
        Action::Clear => ("clear", None, Value::Null),
        Action::Replace(value) => ("replace", None, json!({ "value": value })),
        Action::Select { start, end } => ("select", None, json!({ "start": start, "end": end })),
        Action::SelectAll => ("select_all", None, Value::Null),
        Action::Cursor(position) => ("cursor", None, json!({ "position": position })),
        Action::CursorFront => ("cursor_front", None, Value::Null),
        Action::CursorEnd => ("cursor_end", None, Value::Null),
        Action::Composition(phase) => ("composition", None, composition_value(phase)),
        Action::Key(key) => ("key", None, json!({ "key": encoded_key(key) })),
        Action::KeyDown { key, metadata } => (
            "key_down",
            None,
            json!({ "key": encoded_key(key), "metadata": key_metadata_value(metadata) }),
        ),
        Action::KeyUp { key, metadata } => (
            "key_up",
            None,
            json!({ "key": encoded_key(key), "metadata": key_metadata_value(metadata) }),
        ),
        Action::Modifiers(modifiers) => ("modifiers", None, modifier_value(*modifiers)),
        Action::Chord { modifiers, key } => (
            "chord",
            None,
            json!({ "modifiers": modifier_value(*modifiers), "key": encoded_key(key) }),
        ),
        Action::Repeat { key, count } => (
            "repeat",
            None,
            json!({ "key": encoded_key(key), "count": count }),
        ),
        Action::Touch {
            phase,
            id,
            position,
        } => (
            "touch",
            None,
            json!({ "phase": format!("{phase:?}").to_lowercase(), "id": id, "x": position.x, "y": position.y }),
        ),
        Action::Tap { target, count } => ("tap", Some(target.clone()), json!({ "count": count })),
        Action::WindowOpened => ("window_opened", None, Value::Null),
        Action::WindowClosed => ("window_closed", None, Value::Null),
        Action::WindowMove(position) => (
            "window_move",
            None,
            json!({ "x": position.x, "y": position.y }),
        ),
        Action::Resize(size) => (
            "resize",
            None,
            json!({ "width": size.width, "height": size.height }),
        ),
        Action::Rescale(scale) => ("rescale", None, json!({ "scale": scale })),
        Action::CloseRequested => ("close_requested", None, Value::Null),
        Action::Redraw => ("redraw", None, Value::Null),
        Action::SystemTheme(theme) => (
            "system_theme",
            None,
            json!({ "theme": theme_mode_name(*theme) }),
        ),
        Action::FileHover(path) => ("file_hover", None, json!({ "path": path })),
        Action::FileDrop(path) => ("file_drop", None, json!({ "path": path })),
        Action::FileLeave => ("file_leave", None, Value::Null),
        Action::Wait(duration) => ("wait", None, json!({ "duration_ms": duration.as_millis() })),
        Action::Advance(duration) => (
            "advance",
            None,
            json!({ "duration_ms": duration.as_millis() }),
        ),
        Action::Idle => ("idle", None, Value::Null),
        Action::Capture(name) => ("capture", None, json!({ "name": name })),
        Action::Accessibility { action, target } => (
            "accessibility",
            Some(target.clone()),
            json!({ "action": format!("{action:?}").to_lowercase() }),
        ),
    };
    RecordedAction {
        index,
        kind: kind.into(),
        target,
        parameters,
        source: source_location(source),
        target_source: target_source.map(source_location),
    }
}

fn environment<P>(driver: &Driver<P>, config: &Config) -> Environment
where
    P: Program + 'static,
    P::Renderer: 'static,
    P::Message: Clone,
{
    Environment {
        preset: config.preset.map(str::to_owned),
        viewport_width: driver.size.width,
        viewport_height: driver.size.height,
        theme: driver
            .theme_override
            .map(theme_mode_name)
            .map(str::to_owned),
        system_theme: theme_mode_name(driver.system_theme).into(),
        scale_factor: driver.scale_factor(),
        locale: driver.locale.map(str::to_owned),
        platform: platform_name(driver.platform).into(),
        reduced_motion: driver.reduced_motion,
        build_profile: if cfg!(debug_assertions) {
            "debug"
        } else {
            "release"
        }
        .into(),
    }
}

pub(super) fn summaries(samples: &[Sample]) -> Vec<Summary> {
    let mut grouped = std::collections::BTreeMap::<(usize, Phase), Vec<u64>>::new();
    for sample in samples {
        grouped
            .entry((sample.action_index, sample.phase))
            .or_default()
            .push(sample.duration_ns);
    }
    grouped
        .into_iter()
        .map(|((action_index, phase), mut values)| {
            values.sort_unstable();
            Summary {
                action_index,
                phase,
                samples: values.len(),
                p50_ns: percentile(&values, 50),
                p95_ns: percentile(&values, 95),
                p99_ns: percentile(&values, 99),
                max_ns: *values.last().expect("sample group is non-empty"),
                deadline_misses_60hz: deadline_misses(&values, 60),
                deadline_misses_120hz: deadline_misses(&values, 120),
            }
        })
        .collect()
}

fn percentile(values: &[u64], rank: usize) -> u64 {
    let index = (values.len() * rank).div_ceil(100).saturating_sub(1);
    values[index]
}

fn deadline_misses(values: &[u64], frames_per_second: u64) -> usize {
    let deadline = 1_000_000_000 / frames_per_second;
    values.iter().filter(|value| **value > deadline).count()
}

fn duration_ns(duration: Duration) -> u64 {
    u64::try_from(duration.as_nanos()).unwrap_or(u64::MAX)
}

fn mouse_button(button: MouseButton) -> Value {
    match button {
        MouseButton::Left => json!("left"),
        MouseButton::Right => json!("right"),
        MouseButton::Middle => json!("middle"),
        MouseButton::Back => json!("back"),
        MouseButton::Forward => json!("forward"),
        MouseButton::Other(value) => json!({ "other": value }),
    }
}

fn encoded_key(key: &Key) -> Value {
    match key {
        Key::Named(name) => json!({ "named": format!("{name:?}") }),
        Key::Character(value) => json!({ "character": value }),
        Key::Unidentified => json!({ "unidentified": true }),
    }
}

fn modifier_value(modifiers: Modifiers) -> Value {
    json!({
        "shift": modifiers.shift,
        "control": modifiers.control,
        "alt": modifiers.alt,
        "logo": modifiers.logo,
    })
}

fn composition_value(phase: &CompositionPhase) -> Value {
    match phase {
        CompositionPhase::Start => json!({ "phase": "start" }),
        CompositionPhase::Update { text, selection } => json!({
            "phase": "update",
            "text": text,
            "selection": selection.as_ref().map(|range| [range.start, range.end]),
        }),
        CompositionPhase::Commit(text) => json!({ "phase": "commit", "text": text }),
        CompositionPhase::Cancel => json!({ "phase": "cancel" }),
    }
}

fn key_metadata_value(metadata: &KeyMetadata) -> Value {
    json!({
        "modified_key": metadata.modified_key.as_ref().map(encoded_key),
        "physical_key": metadata.physical_key.map(physical_key_value),
        "location": match metadata.location {
            KeyLocation::Standard => "standard",
            KeyLocation::Left => "left",
            KeyLocation::Right => "right",
            KeyLocation::Numpad => "numpad",
        },
        "text": metadata.text,
        "repeat": metadata.repeat,
    })
}

fn physical_key_value(physical: keyboard::key::Physical) -> Value {
    use keyboard::key::{NativeCode, Physical};
    match physical {
        Physical::Code(code) => json!({ "code": format!("{code:?}") }),
        Physical::Unidentified(native) => match native {
            NativeCode::Unidentified => json!({ "native": { "kind": "unidentified" } }),
            NativeCode::Android(code) => json!({ "native": { "kind": "android", "code": code } }),
            NativeCode::MacOS(code) => json!({ "native": { "kind": "macos", "code": code } }),
            NativeCode::Windows(code) => json!({ "native": { "kind": "windows", "code": code } }),
            NativeCode::Xkb(code) => json!({ "native": { "kind": "xkb", "code": code } }),
        },
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn summaries_retain_raw_distribution_deadlines_and_tail_quantiles() {
        let samples = [1, 2, 3, 4, 20_000_000]
            .into_iter()
            .map(|duration_ns| Sample {
                run: 0,
                action_index: 0,
                phase: Phase::Action,
                duration_ns,
            })
            .collect::<Vec<_>>();
        let summary = summaries(&samples).pop().unwrap();
        assert_eq!(summary.p50_ns, 3);
        assert_eq!(summary.p95_ns, 20_000_000);
        assert_eq!(summary.p99_ns, 20_000_000);
        assert_eq!(summary.max_ns, 20_000_000);
        assert_eq!(summary.deadline_misses_60hz, 1);
        assert_eq!(summary.deadline_misses_120hz, 1);
    }

    #[test]
    fn generator_version_and_seed_fix_the_semantic_action_sequence() {
        let inventory = vec![
            InteractionTarget {
                id: "App/button".into(),
                visible: true,
                scrollable: false,
                focusable: true,
            },
            InteractionTarget {
                id: "App/list".into(),
                visible: true,
                scrollable: true,
                focusable: false,
            },
        ];
        let generate = || {
            let mut generator = Generator::new(18_421);
            (0..50)
                .map(|index| {
                    let action = generator.next(&inventory, Size::new(800.0, 600.0));
                    record_action(
                        index,
                        &action,
                        Location::new("app.ice", 1, 1, "generated"),
                        None,
                    )
                })
                .collect::<Vec<_>>()
        };
        assert_eq!(GENERATOR_VERSION, 1);
        assert_eq!(generate(), generate());
    }

    #[test]
    fn generated_policy_actions_round_trip_through_the_replay_model() {
        let inventory = vec![InteractionTarget {
            id: "App/list".into(),
            visible: true,
            scrollable: true,
            focusable: true,
        }];
        let mut generator = Generator::new(7);
        for index in 0..100 {
            let action = generator.next(&inventory, Size::new(800.0, 600.0));
            let recorded = record_action(
                index,
                &action,
                Location::new("app.ice", 1, 1, "generated"),
                None,
            );
            let replayed = replay_action(&recorded).unwrap();
            assert_eq!(
                record_action(
                    index,
                    &replayed,
                    Location::new("app.ice", 1, 1, "generated"),
                    None,
                ),
                recorded
            );
        }
    }

    #[test]
    fn structured_authored_actions_round_trip_without_debug_string_parsing() {
        let actions = [
            Action::Drag {
                from: "App/source".into(),
                to: "App/destination".into(),
            },
            Action::DropAt("App/destination".into()),
            Action::Composition(CompositionPhase::Update {
                text: "한글".into(),
                selection: Some(1..2),
            }),
            Action::KeyDown {
                key: Key::character("a"),
                metadata: KeyMetadata {
                    modified_key: Some(Key::character("A")),
                    physical_key: Some(keyboard::key::Physical::Code(keyboard::key::Code::KeyA)),
                    location: KeyLocation::Left,
                    text: Some("a".into()),
                    repeat: true,
                },
            },
            Action::KeyUp {
                key: Key::character("a"),
                metadata: KeyMetadata {
                    physical_key: Some(keyboard::key::Physical::Unidentified(
                        keyboard::key::NativeCode::Xkb(38),
                    )),
                    ..KeyMetadata::default()
                },
            },
            Action::Touch {
                phase: TouchPhase::Move,
                id: 42,
                position: Point::new(12.5, 18.25),
            },
        ];
        for (index, action) in actions.iter().enumerate() {
            let recorded = record_action(
                index,
                action,
                Location::new("app.ice", 1, 1, "authored"),
                None,
            );
            let replayed = replay_action(&recorded).unwrap();
            assert_eq!(
                record_action(
                    index,
                    &replayed,
                    Location::new("app.ice", 1, 1, "authored"),
                    None,
                ),
                recorded
            );
        }
    }

    #[test]
    fn finding_fingerprint_distinguishes_semantic_targets() {
        let action = |target: &str| RecordedAction {
            index: 0,
            kind: "click".into(),
            target: Some(target.into()),
            parameters: Value::Null,
            source: SourceLocation {
                path: "app.ice".into(),
                line: 1,
                column: 1,
                statement: "click".into(),
            },
            target_source: None,
        };
        assert_ne!(
            fingerprint(
                FindingKind::Latency,
                &action_identity(&action("App/a")),
                Some(Phase::Action),
                ""
            ),
            fingerprint(
                FindingKind::Latency,
                &action_identity(&action("App/b")),
                Some(Phase::Action),
                ""
            )
        );
    }
}
