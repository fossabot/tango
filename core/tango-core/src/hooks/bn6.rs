use crate::{facade, fastforwarder, hooks, input, shadow};

mod munger;
mod offsets;

#[derive(Clone)]
pub struct BN6 {
    offsets: offsets::Offsets,
    munger: munger::Munger,
}

lazy_static! {
    pub static ref MEGAMAN6_FXX: Box<dyn hooks::Hooks + Send + Sync> =
        BN6::new(offsets::MEGAMAN6_FXX);
    pub static ref MEGAMAN6_GXX: Box<dyn hooks::Hooks + Send + Sync> =
        BN6::new(offsets::MEGAMAN6_GXX);
    pub static ref ROCKEXE6_RXX: Box<dyn hooks::Hooks + Send + Sync> =
        BN6::new(offsets::ROCKEXE6_RXX);
    pub static ref ROCKEXE6_GXX: Box<dyn hooks::Hooks + Send + Sync> =
        BN6::new(offsets::ROCKEXE6_GXX);
}

impl BN6 {
    pub fn new(offsets: offsets::Offsets) -> Box<dyn hooks::Hooks + Send + Sync> {
        Box::new(BN6 {
            offsets,
            munger: munger::Munger { offsets },
        })
    }
}

fn generate_rng1_state(rng: &mut impl rand::Rng) -> u32 {
    let mut rng1_state = 0;
    for _ in 0..rng.gen_range(0..=0xffffusize) {
        rng1_state = step_rng(rng1_state);
    }
    rng1_state
}

fn generate_rng2_state(rng: &mut impl rand::Rng) -> u32 {
    let mut rng2_state = 0xa338244f;
    for _ in 0..rng.gen_range(0..=0xffffusize) {
        rng2_state = step_rng(rng2_state);
    }
    rng2_state
}

fn random_battle_settings_and_background(rng: &mut impl rand::Rng, match_type: u8) -> u16 {
    const BATTLE_BACKGROUNDS: &[u16] = &[
        0x00, 0x01, 0x01, 0x03, 0x04, 0x05, 0x06, 0x07, 0x08, 0x09, 0x0a, 0x0b, 0x0c, 0x0d, 0x0e,
        0x0f, 0x10, 0x11, 0x11, 0x13, 0x13,
    ];

    let lo = match match_type {
        0 => rng.gen_range(0..0x44u16),
        1 => rng.gen_range(0..0x60u16),
        2 => rng.gen_range(0..0x44u16) + 0x60u16,
        _ => 0u16,
    };

    let hi = BATTLE_BACKGROUNDS[rng.gen_range(0..BATTLE_BACKGROUNDS.len())];

    hi << 0x8 | lo
}

fn step_rng(seed: u32) -> u32 {
    let seed = std::num::Wrapping(seed);
    (((seed * std::num::Wrapping(2)) - (seed >> 0x1f) + std::num::Wrapping(1))
        ^ std::num::Wrapping(0x873ca9e5))
    .0
}

impl hooks::Hooks for BN6 {
    fn primary_traps(
        &self,
        handle: tokio::runtime::Handle,
        joyflags: std::sync::Arc<std::sync::atomic::AtomicU32>,
        facade: facade::Facade,
    ) -> Vec<(u32, Box<dyn FnMut(mgba::core::CoreMutRef)>)> {
        vec![
            {
                let munger = self.munger.clone();
                (
                    self.offsets.rom.start_screen_jump_table_entry,
                    Box::new(move |core| {
                        munger.skip_logo(core);
                    }),
                )
            },
            {
                let munger = self.munger.clone();
                (
                    self.offsets.rom.start_screen_sram_unmask_ret,
                    Box::new(move |core| {
                        munger.continue_from_title_menu(core);
                    }),
                )
            },
            {
                let munger = self.munger.clone();
                (
                    self.offsets.rom.game_load_ret,
                    Box::new(move |core| {
                        munger.open_comm_menu_from_overworld(core);
                    }),
                )
            },
            {
                let facade = facade.clone();
                let handle = handle.clone();
                let munger = self.munger.clone();
                (
                    self.offsets.rom.comm_menu_init_ret,
                    Box::new(move |core| {
                        handle.block_on(async {
                            let match_ = match facade.match_().await {
                                Some(match_) => match_,
                                None => {
                                    return;
                                }
                            };

                            munger.start_battle_from_comm_menu(core, match_.match_type());

                            let mut rng = match_.lock_rng().await;

                            // rng1 is the local rng, it should not be synced.
                            // However, we should make sure it's reproducible from the shared RNG state so we generate it like this.
                            let offerer_rng1_state = generate_rng1_state(&mut *rng);
                            let answerer_rng1_state = generate_rng1_state(&mut *rng);
                            munger.set_rng1_state(
                                core,
                                if match_.is_offerer() {
                                    offerer_rng1_state
                                } else {
                                    answerer_rng1_state
                                },
                            );

                            // rng2 is the shared rng, it must be synced.
                            munger.set_rng2_state(core, generate_rng2_state(&mut *rng));
                        });
                    }),
                )
            },
            {
                let facade = facade.clone();
                let handle = handle.clone();
                (
                    self.offsets.rom.round_run_unpaused_step_cmp_retval,
                    Box::new(move |core| {
                        handle.block_on(async {
                            let match_ = match facade.match_().await {
                                Some(match_) => match_,
                                None => {
                                    return;
                                }
                            };

                            let mut round_state = match_.lock_round_state().await;

                            match core.as_ref().gba().cpu().gpr(0) {
                                1 => {
                                    round_state.set_won_last_round(true);
                                }
                                2 => {
                                    round_state.set_won_last_round(false);
                                }
                                _ => {}
                            }
                        });
                    }),
                )
            },
            {
                let facade = facade.clone();
                let handle = handle.clone();
                (
                    self.offsets.rom.round_end_entry,
                    Box::new(move |_| {
                        handle.block_on(async {
                            let match_ = match facade.match_().await {
                                Some(match_) => match_,
                                None => {
                                    return;
                                }
                            };

                            let mut round_state = match_.lock_round_state().await;
                            round_state.end_round().await.expect("end round");

                            match_
                                .advance_shadow_until_round_end()
                                .await
                                .expect("advance shadow");
                        });
                    }),
                )
            },
            {
                let facade = facade.clone();
                let handle = handle.clone();
                (
                    self.offsets.rom.round_start_ret,
                    Box::new(move |core| {
                        handle.block_on(async {
                            let match_ = match facade.match_().await {
                                Some(match_) => match_,
                                None => {
                                    return;
                                }
                            };

                            match_.start_round(core).await.expect("start round");
                        });
                    }),
                )
            },
            {
                let facade = facade.clone();
                let handle = handle.clone();
                (
                    self.offsets.rom.battle_is_p2_tst,
                    Box::new(move |mut core| {
                        handle.block_on(async {
                            let match_ = match facade.match_().await {
                                Some(match_) => match_,
                                None => {
                                    return;
                                }
                            };

                            let round_state = match_.lock_round_state().await;
                            let round = round_state.round.as_ref().expect("round");

                            core.gba_mut()
                                .cpu_mut()
                                .set_gpr(0, round.local_player_index() as i32);
                        });
                    }),
                )
            },
            {
                let facade = facade.clone();
                let handle = handle.clone();
                (
                    self.offsets.rom.link_is_p2_ret,
                    Box::new(move |mut core| {
                        handle.block_on(async {
                            let match_ = match facade.match_().await {
                                Some(match_) => match_,
                                None => {
                                    return;
                                }
                            };

                            let round_state = match_.lock_round_state().await;
                            let round = round_state.round.as_ref().expect("round");

                            core.gba_mut()
                                .cpu_mut()
                                .set_gpr(0, round.local_player_index() as i32);
                        });
                    }),
                )
            },
            {
                let facade = facade.clone();
                let handle = handle.clone();
                (
                    self.offsets.rom.get_copy_data_input_state_ret,
                    Box::new(move |mut core| {
                        handle.block_on(async {
                            let mut r0 = core.as_ref().gba().cpu().gpr(0);
                            if r0 != 2 {
                                log::error!("expected r0 to be 2 but got {}", r0);
                            }

                            if facade.match_().await.is_none() {
                                r0 = 4;
                            }

                            core.gba_mut().cpu_mut().set_gpr(0, r0);
                        });
                    }),
                )
            },
            {
                (
                    self.offsets.rom.comm_menu_handle_link_cable_input_entry,
                    Box::new(move |core| {
                        log::error!(
                            "unhandled call to commMenu_handleLinkCableInput at 0x{:0x}: uh oh!",
                            core.as_ref().gba().cpu().gpr(15) - 4
                        );
                    }),
                )
            },
            {
                let facade = facade.clone();
                let munger = self.munger.clone();
                let handle = handle.clone();
                (
                    self.offsets.rom.comm_menu_init_battle_entry,
                    Box::new(move |core| {
                        handle.block_on(async {
                            let match_ = match facade.match_().await {
                                Some(match_) => match_,
                                None => {
                                    return;
                                }
                            };

                            let mut rng = match_.lock_rng().await;
                            munger.set_link_battle_settings_and_background(
                                core,
                                random_battle_settings_and_background(
                                    &mut *rng,
                                    (match_.match_type() & 0xff) as u8,
                                ),
                            );
                        });
                    }),
                )
            },
            {
                let facade = facade.clone();
                let handle = handle.clone();
                (
                    self.offsets.rom.comm_menu_end_battle_entry,
                    Box::new(move |_core| {
                        handle.block_on(async {
                            log::info!("match ended");
                            facade.end_match().await;
                        });
                    }),
                )
            },
            {
                (
                    self.offsets
                        .rom
                        .comm_menu_in_battle_call_comm_menu_handle_link_cable_input,
                    Box::new(move |mut core| {
                        let r15 = core.as_ref().gba().cpu().gpr(15) as u32;
                        core.gba_mut().cpu_mut().set_pc(r15 + 4);
                    }),
                )
            },
            {
                let facade = facade.clone();
                let munger = self.munger.clone();
                let handle = handle.clone();
                (
                    self.offsets.rom.main_read_joyflags,
                    Box::new(move |core| {
                        handle.block_on(async {
                            'abort: loop {
                                let match_ = match facade.match_().await {
                                    Some(match_) => match_,
                                    None => {
                                        return;
                                    }
                                };

                                let mut round_state = match_.lock_round_state().await;

                                let round = match round_state.round.as_mut() {
                                    Some(round) => round,
                                    None => {
                                        return;
                                    }
                                };

                                let current_tick = munger.current_tick(core);
                                if !round.has_committed_state() {
                                    round.set_first_committed_state(
                                        core.save_state().expect("save state"),
                                        match_
                                            .advance_shadow_until_first_committed_state()
                                            .await
                                            .expect("shadow save state"),
                                    );
                                    round.fill_input_delay(current_tick);
                                    log::info!(
                                        "primary rng1 state: {:08x}",
                                        munger.rng1_state(core)
                                    );
                                    log::info!(
                                        "primary rng2 state: {:08x}",
                                        munger.rng2_state(core)
                                    );
                                    log::info!("battle state committed on {}", current_tick);
                                }

                                if !round
                                    .add_local_input_and_fastforward(
                                        core,
                                        current_tick,
                                        joyflags.load(std::sync::atomic::Ordering::Relaxed) as u16,
                                        munger.tx_packet(core).to_vec(),
                                    )
                                    .await
                                {
                                    break 'abort;
                                }
                                return;
                            }
                            facade.abort_match().await;
                        });
                    }),
                )
            },
            {
                let facade = facade.clone();
                let munger = self.munger.clone();
                let handle = handle.clone();
                (
                    self.offsets.rom.copy_input_data_entry,
                    Box::new(move |core| {
                        handle.block_on(async {
                            let match_ = match facade.match_().await {
                                Some(match_) => match_,
                                None => {
                                    return;
                                }
                            };

                            let mut round_state = match_.lock_round_state().await;

                            let round = if let Some(round) = round_state.round.as_mut() {
                                round
                            } else {
                                return;
                            };

                            let ip = if let Some(ip) = round.take_last_input() {
                                ip
                            } else {
                                return;
                            };

                            munger.set_rx_packet(
                                core,
                                round.local_player_index() as u32,
                                &ip.local.rx.try_into().unwrap(),
                            );

                            munger.set_rx_packet(
                                core,
                                round.remote_player_index() as u32,
                                &ip.remote.rx.try_into().unwrap(),
                            );
                        })
                    }),
                )
            },
        ]
    }

    fn shadow_traps(
        &self,
        shadow_state: shadow::State,
    ) -> Vec<(u32, Box<dyn FnMut(mgba::core::CoreMutRef)>)> {
        vec![
            {
                let munger = self.munger.clone();
                (
                    self.offsets.rom.start_screen_jump_table_entry,
                    Box::new(move |core| {
                        munger.skip_logo(core);
                    }),
                )
            },
            {
                let munger = self.munger.clone();
                (
                    self.offsets.rom.start_screen_sram_unmask_ret,
                    Box::new(move |core| {
                        munger.continue_from_title_menu(core);
                    }),
                )
            },
            {
                let munger = self.munger.clone();
                (
                    self.offsets.rom.game_load_ret,
                    Box::new(move |core| {
                        munger.open_comm_menu_from_overworld(core);
                    }),
                )
            },
            {
                let munger = self.munger.clone();
                let shadow_state = shadow_state.clone();
                (
                    self.offsets.rom.comm_menu_init_ret,
                    Box::new(move |core| {
                        munger.start_battle_from_comm_menu(core, shadow_state.match_type());

                        let mut rng = shadow_state.lock_rng();

                        // rng1 is the local rng, it should not be synced.
                        // However, we should make sure it's reproducible from the shared RNG state so we generate it like this.
                        let offerer_rng1_state = generate_rng1_state(&mut *rng);
                        let answerer_rng1_state = generate_rng1_state(&mut *rng);
                        munger.set_rng1_state(
                            core,
                            if shadow_state.is_offerer() {
                                answerer_rng1_state
                            } else {
                                offerer_rng1_state
                            },
                        );

                        // rng2 is the shared rng, it must be synced.
                        munger.set_rng2_state(core, generate_rng2_state(&mut *rng));
                    }),
                )
            },
            {
                let shadow_state = shadow_state.clone();
                (
                    self.offsets.rom.round_run_unpaused_step_cmp_retval,
                    Box::new(move |core| {
                        match core.as_ref().gba().cpu().gpr(0) {
                            1 => {
                                shadow_state.set_won_last_round(false);
                            }
                            2 => {
                                shadow_state.set_won_last_round(true);
                            }
                            _ => {}
                        };
                    }),
                )
            },
            {
                let shadow_state = shadow_state.clone();
                (
                    self.offsets.rom.round_start_ret,
                    Box::new(move |_| {
                        shadow_state.start_round();
                    }),
                )
            },
            {
                let shadow_state = shadow_state.clone();
                (
                    self.offsets.rom.round_end_entry,
                    Box::new(move |core| {
                        shadow_state.end_round();
                        shadow_state.set_applied_state(core.save_state().expect("save state"));
                    }),
                )
            },
            {
                let shadow_state = shadow_state.clone();
                (
                    self.offsets.rom.battle_is_p2_tst,
                    Box::new(move |mut core| {
                        let mut round_state = shadow_state.lock_round_state();
                        let round = round_state.round.as_mut().expect("round");

                        core.gba_mut()
                            .cpu_mut()
                            .set_gpr(0, round.remote_player_index() as i32);
                    }),
                )
            },
            {
                let shadow_state = shadow_state.clone();
                (
                    self.offsets.rom.link_is_p2_ret,
                    Box::new(move |mut core| {
                        let mut round_state = shadow_state.lock_round_state();
                        let round = round_state.round.as_mut().expect("round");

                        core.gba_mut()
                            .cpu_mut()
                            .set_gpr(0, round.remote_player_index() as i32);
                    }),
                )
            },
            {
                (
                    self.offsets.rom.get_copy_data_input_state_ret,
                    Box::new(move |core| {
                        let r0 = core.as_ref().gba().cpu().gpr(0);
                        if r0 != 2 {
                            log::error!("shadow: expected r0 to be 2 but got {}", r0);
                        }
                    }),
                )
            },
            {
                (
                    self.offsets.rom.comm_menu_handle_link_cable_input_entry,
                    Box::new(move |core| {
                        log::error!(
                            "unhandled call to commMenu_handleLinkCableInput at 0x{:0x}: uh oh!",
                            core.as_ref().gba().cpu().gpr(15) - 4
                        );
                    }),
                )
            },
            {
                let shadow_state = shadow_state.clone();
                let munger = self.munger.clone();
                (
                    self.offsets.rom.comm_menu_init_battle_entry,
                    Box::new(move |core| {
                        let mut rng = shadow_state.lock_rng();
                        munger.set_link_battle_settings_and_background(
                            core,
                            random_battle_settings_and_background(
                                &mut *rng,
                                (shadow_state.match_type() & 0xff) as u8,
                            ),
                        );
                    }),
                )
            },
            {
                (
                    self.offsets
                        .rom
                        .comm_menu_in_battle_call_comm_menu_handle_link_cable_input,
                    Box::new(move |mut core| {
                        let r15 = core.as_ref().gba().cpu().gpr(15) as u32;
                        core.gba_mut().cpu_mut().set_pc(r15 + 4);
                    }),
                )
            },
            {
                let shadow_state = shadow_state.clone();
                let munger = self.munger.clone();
                (
                    self.offsets.rom.main_read_joyflags,
                    Box::new(move |mut core| {
                        let current_tick = munger.current_tick(core);

                        let mut round_state = shadow_state.lock_round_state();
                        let round = match round_state.round.as_mut() {
                            Some(round) => round,
                            None => {
                                return;
                            }
                        };

                        if !round.has_first_committed_state() {
                            round.set_first_committed_state(core.save_state().expect("save state"));
                            log::info!("shadow rng1 state: {:08x}", munger.rng1_state(core));
                            log::info!("shadow rng2 state: {:08x}", munger.rng2_state(core));
                            log::info!("shadow state committed on {}", current_tick);
                            return;
                        }

                        if let Some(ip) = round.take_in_input_pair() {
                            if ip.local.local_tick != ip.remote.local_tick {
                                shadow_state.set_anyhow_error(anyhow::anyhow!(
                                    "read joyflags: local tick != remote tick (in battle tick = {}): {} != {}",
                                    current_tick,
                                    ip.local.local_tick,
                                    ip.remote.local_tick
                                ));
                                return;
                            }

                            if ip.local.local_tick != current_tick {
                                shadow_state.set_anyhow_error(anyhow::anyhow!(
                                    "read joyflags: input tick != in battle tick: {} != {}",
                                    ip.local.local_tick,
                                    current_tick,
                                ));
                                return;
                            }

                            round.set_out_input_pair(input::Pair {
                                local: ip.local,
                                remote: input::Input {
                                    local_tick: ip.remote.local_tick,
                                    remote_tick: ip.remote.remote_tick,
                                    joyflags: ip.remote.joyflags,
                                    rx: munger.tx_packet(core).to_vec(),
                                },
                            });

                            core.gba_mut()
                                .cpu_mut()
                                .set_gpr(4, (ip.remote.joyflags | 0xfc00) as i32);
                        }

                        if round.take_input_injected() {
                            shadow_state.set_applied_state(core.save_state().expect("save state"));
                        }
                    }),
                )
            },
            {
                let shadow_state = shadow_state.clone();
                let munger = self.munger.clone();
                (
                    self.offsets.rom.copy_input_data_entry,
                    Box::new(move |core| {
                        let current_tick = munger.current_tick(core);

                        let mut round_state = shadow_state.lock_round_state();
                        let round = round_state.round.as_mut().expect("round");

                        let ip = if let Some(ip) = round.peek_out_input_pair().as_ref() {
                            ip
                        } else {
                            return;
                        };

                        // HACK: This is required if the emulator advances beyond read joyflags and runs this function again, but is missing input data.
                        // We permit this for one tick only, but really we should just not be able to get into this situation in the first place.
                        if ip.local.local_tick + 1 == current_tick {
                            return;
                        }

                        if ip.local.local_tick != ip.remote.local_tick {
                            shadow_state.set_anyhow_error(anyhow::anyhow!(
                                "copy input data: local tick != remote tick (in battle tick = {}): {} != {}",
                                current_tick,
                                ip.local.local_tick,
                                ip.remote.local_tick
                            ));
                            return;
                        }

                        if ip.local.local_tick != current_tick {
                            shadow_state.set_anyhow_error(anyhow::anyhow!(
                                "copy input data: input tick != in battle tick: {} != {}",
                                ip.local.local_tick,
                                current_tick,
                            ));
                            return;
                        }

                        munger.set_rx_packet(
                            core,
                            round.local_player_index() as u32,
                            &ip.local.rx.clone().try_into().unwrap(),
                        );

                        munger.set_rx_packet(
                            core,
                            round.remote_player_index() as u32,
                            &ip.remote.rx.clone().try_into().unwrap(),
                        );

                        round.set_input_injected();
                    }),
                )
            },
        ]
    }

    fn fastforwarder_traps(
        &self,
        ff_state: fastforwarder::State,
    ) -> Vec<(u32, Box<dyn FnMut(mgba::core::CoreMutRef)>)> {
        vec![
            {
                let ff_state = ff_state.clone();
                (
                    self.offsets.rom.battle_is_p2_tst,
                    Box::new(move |mut core| {
                        core.gba_mut()
                            .cpu_mut()
                            .set_gpr(0, ff_state.local_player_index() as i32);
                    }),
                )
            },
            {
                let ff_state = ff_state.clone();
                (
                    self.offsets.rom.link_is_p2_ret,
                    Box::new(move |mut core| {
                        core.gba_mut()
                            .cpu_mut()
                            .set_gpr(0, ff_state.local_player_index() as i32);
                    }),
                )
            },
            {
                (
                    self.offsets
                        .rom
                        .comm_menu_in_battle_call_comm_menu_handle_link_cable_input,
                    Box::new(move |mut core| {
                        let r15 = core.as_ref().gba().cpu().gpr(15) as u32;
                        core.gba_mut().cpu_mut().set_pc(r15 + 4);
                    }),
                )
            },
            {
                (
                    self.offsets.rom.get_copy_data_input_state_ret,
                    Box::new(move |mut core| {
                        core.gba_mut().cpu_mut().set_gpr(0, 2);
                    }),
                )
            },
            {
                let munger = self.munger.clone();
                let ff_state = ff_state.clone();
                (
                    self.offsets.rom.main_read_joyflags,
                    Box::new(move |mut core| {
                        let current_tick = munger.current_tick(core);

                        if current_tick == ff_state.commit_time() {
                            ff_state.set_committed_state(
                                core.save_state().expect("save committed state"),
                            );
                        }

                        let ip = match ff_state.peek_input_pair() {
                            Some(ip) => ip,
                            None => {
                                ff_state.on_inputs_exhausted();
                                return;
                            }
                        };

                        if ip.local.local_tick != ip.remote.local_tick {
                            ff_state.set_anyhow_error(anyhow::anyhow!(
                                "read joyflags: local tick != remote tick (in battle tick = {}): {} != {}",
                                current_tick,
                                ip.local.local_tick,
                                ip.remote.local_tick
                            ));
                            return;
                        }

                        if ip.local.local_tick != current_tick {
                            ff_state.set_anyhow_error(anyhow::anyhow!(
                                "read joyflags: input tick != in battle tick: {} != {}",
                                ip.local.local_tick,
                                current_tick,
                            ));
                            return;
                        }

                        core.gba_mut()
                            .cpu_mut()
                            .set_gpr(4, (ip.local.joyflags | 0xfc00) as i32);

                        if current_tick == ff_state.dirty_time() {
                            ff_state.set_dirty_state(core.save_state().expect("save dirty state"));
                        }
                    }),
                )
            },
            {
                let munger = self.munger.clone();
                let ff_state = ff_state.clone();
                (
                    self.offsets.rom.copy_input_data_entry,
                    Box::new(move |core| {
                        let current_tick = munger.current_tick(core);

                        let ip = match ff_state.pop_input_pair() {
                            Some(ip) => ip,
                            None => {
                                return;
                            }
                        };

                        if ip.local.local_tick != ip.remote.local_tick {
                            ff_state.set_anyhow_error(anyhow::anyhow!(
                                "copy input data: local tick != remote tick (in battle tick = {}): {} != {}",
                                current_tick,
                                ip.local.local_tick,
                                ip.local.local_tick
                            ));
                            return;
                        }

                        if ip.local.local_tick != current_tick {
                            ff_state.set_anyhow_error(anyhow::anyhow!(
                                "copy input data: input tick != in battle tick: {} != {}",
                                ip.local.local_tick,
                                current_tick,
                            ));
                            return;
                        }

                        munger.set_rx_packet(
                            core,
                            ff_state.local_player_index() as u32,
                            &ip.local.rx.try_into().unwrap(),
                        );

                        munger.set_rx_packet(
                            core,
                            ff_state.remote_player_index() as u32,
                            &ip.remote.rx.try_into().unwrap(),
                        );
                    }),
                )
            },
        ]
    }

    fn placeholder_rx(&self) -> Vec<u8> {
        vec![0; 0x10]
    }

    fn prepare_for_fastforward(&self, mut core: mgba::core::CoreMutRef) {
        core.gba_mut()
            .cpu_mut()
            .set_pc(self.offsets.rom.main_read_joyflags);
    }

    fn replace_opponent_name(&self, mut core: mgba::core::CoreMutRef, name: &str) {
        if self.offsets.rom.opponent_name == 0 {
            // Not whimsical enough :(
            return;
        }
        if name.is_empty() {
            return;
        }
        const CHARS: &str = " 0123456789ABCDEFGHIJKLMNOPQRSTUVWXYZ*abcdefghijklmnopqrstuvwxyz";
        const MAX_NAME_LEN: usize = 9;
        const EOS: u8 = 0xe6;
        let mut buf = Vec::with_capacity(MAX_NAME_LEN);
        for c in name.chars() {
            if buf.len() == MAX_NAME_LEN {
                break;
            }

            buf.push(if let Some(i) = CHARS.find(c) {
                i as u8
            } else {
                0
            });
        }
        buf.push(EOS);
        core.raw_write_range(self.offsets.rom.opponent_name, -1, &buf);
    }

    fn current_tick(&self, core: mgba::core::CoreMutRef) -> u32 {
        self.munger.current_tick(core)
    }
}
