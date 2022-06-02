use crate::{hooks, input};

#[derive(Clone)]
pub struct LocalInput {
    pub tick: u32,
    pub rx: Vec<u8>,
}

#[derive(Clone)]
pub struct RemoteInput {
    pub tick: u32,
    pub joyflags: u16,
    pub rx: Vec<u8>,
}

pub struct Output {
    pub tick: u32,
    pub tx: Vec<u8>,
}

pub struct Round {
    local_player_index: u8,
    is_accepting_input: bool,
    committed_state: Option<mgba::state::State>,
    input: Option<input::Pair<LocalInput, RemoteInput>>,
    output: Option<Output>,
}

impl Round {
    pub fn local_player_index(&self) -> u8 {
        self.local_player_index
    }

    pub fn remote_player_index(&self) -> u8 {
        1 - self.local_player_index
    }

    pub fn start_accepting_input(&mut self) {
        self.is_accepting_input = true;
    }

    pub fn is_accepting_input(&mut self) -> bool {
        self.is_accepting_input
    }

    pub fn set_committed_state(&mut self, state: mgba::state::State) {
        self.committed_state = Some(state);
    }

    pub fn take_input(&mut self) -> Option<input::Pair<LocalInput, RemoteInput>> {
        self.input.take()
    }

    pub fn peek_input(&self) -> &Option<input::Pair<LocalInput, RemoteInput>> {
        &self.input
    }

    pub fn take_output(&mut self) -> Option<Output> {
        self.output.take()
    }

    pub fn set_output(&mut self, output: Output) {
        self.output = Some(output);
    }
}

pub struct RoundState {
    pub round: Option<Round>,
    pub won_last_round: bool,
}

struct InnerState {
    match_type: u16,
    is_offerer: bool,
    round_state: parking_lot::Mutex<RoundState>,
    rng: parking_lot::Mutex<rand_pcg::Mcg128Xsl64>,
    error: parking_lot::Mutex<Option<anyhow::Error>>,
}

pub struct Shadow {
    core: mgba::core::Core,
    state: State,
}

#[derive(Clone)]
pub struct State(std::sync::Arc<InnerState>);

impl State {
    pub fn new(
        match_type: u16,
        is_offerer: bool,
        rng: rand_pcg::Mcg128Xsl64,
        won_last_round: bool,
    ) -> State {
        State(std::sync::Arc::new(InnerState {
            match_type,
            is_offerer,
            rng: parking_lot::Mutex::new(rng),
            round_state: parking_lot::Mutex::new(RoundState {
                round: None,
                won_last_round,
            }),
            error: parking_lot::Mutex::new(None),
        }))
    }

    pub fn match_type(&self) -> u16 {
        self.0.match_type
    }

    pub fn is_offerer(&self) -> bool {
        self.0.is_offerer
    }

    pub fn lock_rng(&self) -> parking_lot::MutexGuard<rand_pcg::Mcg128Xsl64> {
        self.0.rng.lock()
    }

    pub fn lock_round_state(&self) -> parking_lot::MutexGuard<'_, RoundState> {
        self.0.round_state.lock()
    }

    pub fn start_round(&self) {
        let mut round_state = self.0.round_state.lock();
        round_state.round = Some(Round {
            local_player_index: if round_state.won_last_round { 0 } else { 1 },
            is_accepting_input: false,
            committed_state: None,
            input: None,
            output: None,
        });
    }

    pub fn end_round(&self) {
        let mut round_state = self.0.round_state.lock();
        round_state.round = None;
    }

    pub fn set_won_last_round(&self, did_win: bool) {
        self.0.round_state.lock().won_last_round = did_win;
    }

    pub fn set_error<E>(&self, err: E)
    where
        E: Into<anyhow::Error>,
    {
        *self.0.error.lock() = Some(err.into());
    }
}

impl Shadow {
    pub fn new(
        rom_path: &std::path::Path,
        save_path: &std::path::Path,
        match_type: u16,
        is_offerer: bool,
        won_last_round: bool,
        rng: rand_pcg::Mcg128Xsl64,
    ) -> anyhow::Result<Self> {
        let mut core = mgba::core::Core::new_gba("tango")?;
        let rom_vf = mgba::vfile::VFile::open(rom_path, mgba::vfile::flags::O_RDONLY)?;
        core.as_mut().load_rom(rom_vf)?;

        log::info!("loaded shadow game: {}", core.as_ref().game_title());

        let save_vf = mgba::vfile::VFile::open(
            save_path,
            mgba::vfile::flags::O_CREAT | mgba::vfile::flags::O_RDWR,
        )?;
        core.as_mut().load_save(save_vf)?;

        let state = State::new(match_type, is_offerer, rng, won_last_round);

        let hooks = hooks::HOOKS.get(&core.as_ref().game_title()).unwrap();

        core.set_traps(hooks.shadow_traps(state.clone()));
        core.as_mut().reset();

        Ok(Shadow { core, state })
    }

    pub fn advance_until_first_committed_state(&mut self) -> anyhow::Result<mgba::state::State> {
        log::info!("advancing shadow until first committed state");
        loop {
            self.core.as_mut().run_loop();
            if let Some(err) = self.state.0.error.lock().take() {
                return Err(err);
            }

            let mut round_state = self.state.lock_round_state();

            let round = if let Some(round) = round_state.round.as_mut() {
                round
            } else {
                continue;
            };

            let state = if let Some(state) = &round.committed_state {
                state
            } else {
                continue;
            };

            self.core.as_mut().load_state(&state).expect("load state");
            log::info!("advanced to committed state!");

            return Ok(state.clone());
        }
    }

    pub fn advance_until_round_end(&mut self) -> anyhow::Result<()> {
        log::info!("advancing shadow until round end");
        loop {
            self.core.as_mut().run_loop();
            if let Some(err) = self.state.0.error.lock().take() {
                return Err(err);
            }

            let round_state = self.state.lock_round_state();
            if round_state.round.is_none() {
                return Ok(());
            }
        }
    }

    pub fn apply_input(
        &mut self,
        current_tick: u32,
        joyflags: u16,
        rx: &[u8],
    ) -> anyhow::Result<Vec<u8>> {
        let output = {
            let mut round_state = self.state.lock_round_state();
            let round = round_state.round.as_mut().expect("round");

            let output = if let Some(output) = round.output.take() {
                output
            } else {
                anyhow::bail!("no output in shadow to take")
            };

            if output.tick != current_tick {
                anyhow::bail!(
                    "shadow apply input: output tick != in battle tick: {} != {}",
                    output.tick,
                    current_tick,
                );
            }

            round.input = Some(input::Pair {
                local: LocalInput {
                    tick: current_tick,
                    rx: rx.to_vec(),
                },
                remote: RemoteInput {
                    tick: current_tick,
                    joyflags,
                    rx: output.tx.to_vec(),
                },
            });
            output
        };

        loop {
            self.core.as_mut().run_loop();
            if let Some(err) = self.state.0.error.lock().take() {
                return Err(err);
            }

            let mut round_state = self.state.lock_round_state();
            let round = round_state.round.as_mut().expect("round");

            let state = if let Some(state) = round.committed_state.take() {
                state
            } else {
                continue;
            };

            self.core.as_mut().load_state(&state)?;
            if round.output.is_some() {
                return Ok(output.tx.clone());
            }
        }
    }
}
