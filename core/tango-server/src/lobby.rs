use futures_util::{SinkExt, StreamExt, TryStreamExt};
use prost::Message;
use rand::Rng;

struct PendingPlayer {
    close_sender: Option<tokio::sync::oneshot::Sender<()>>,
    tx: futures_util::stream::SplitSink<
        hyper_tungstenite::WebSocketStream<hyper::upgrade::Upgraded>,
        tungstenite::Message,
    >,
}

struct Lobby {
    game_info: tango_protos::lobby::GameInfo,
    save_data: Vec<u8>,
    pending_players:
        std::collections::HashMap<String, std::sync::Arc<tokio::sync::Mutex<PendingPlayer>>>,
    creator_tx: futures_util::stream::SplitSink<
        hyper_tungstenite::WebSocketStream<hyper::upgrade::Upgraded>,
        tungstenite::Message,
    >,
}

pub struct Server {
    lobbies: std::sync::Arc<
        tokio::sync::Mutex<
            std::collections::HashMap<String, std::sync::Arc<tokio::sync::Mutex<Lobby>>>,
        >,
    >,
}

fn generate_id() -> String {
    rand::thread_rng()
        .sample_iter(&rand::distributions::Alphanumeric)
        .take(32)
        .map(char::from)
        .collect()
}

impl Server {
    pub fn new() -> Server {
        Server {
            lobbies: std::sync::Arc::new(tokio::sync::Mutex::new(std::collections::HashMap::new())),
        }
    }

    pub async fn handle_create_stream(
        &self,
        ws: hyper_tungstenite::WebSocketStream<hyper::upgrade::Upgraded>,
    ) -> anyhow::Result<()> {
        let (mut tx, mut rx) = ws.split();
        let lobby_id = std::sync::Arc::new(tokio::sync::Mutex::new(None));

        let r = {
            let lobbies = self.lobbies.clone();
            let lobby_id = lobby_id.clone();

            (move || async move {
                let msg = match rx.try_next().await? {
                    Some(tungstenite::Message::Binary(d)) => {
                        tango_protos::lobby::CreateStreamToServerMessage::decode(
                            bytes::Bytes::from(d),
                        )?
                    }
                    Some(tungstenite::Message::Close(_)) | None => {
                        return Ok(());
                    }
                    Some(m) => {
                        anyhow::bail!("unexpected message: {:?}", m);
                    }
                };

                let create_req = match msg {
                    tango_protos::lobby::CreateStreamToServerMessage {
                        which:
                            Some(tango_protos::lobby::create_stream_to_server_message::Which::CreateReq(
                                create_req,
                            )),
                    } => create_req,
                    m => anyhow::bail!("unexpected message: {:?}", m),
                };

                let game_info = if let Some(game_info) = create_req.game_info {
                    game_info
                } else {
                    anyhow::bail!("create request was missing game info");
                };

                let generated_lobby_id = generate_id();

                tx.send(tungstenite::Message::Binary(tango_protos::lobby::CreateStreamToClientMessage {
                    which:
                        Some(tango_protos::lobby::create_stream_to_client_message::Which::CreateResp(
                            tango_protos::lobby::create_stream_to_client_message::CreateResponse {
                                lobby_id: generated_lobby_id.clone(),
                            }
                        )),
                }.encode_to_vec())).await?;

                let lobby = std::sync::Arc::new(tokio::sync::Mutex::new(Lobby {
                    game_info,
                    save_data: create_req.save_data,
                    pending_players: std::collections::HashMap::new(),
                    creator_tx: tx,
                }));
                lobbies.lock().await.insert(
                    generated_lobby_id.clone(),
                    lobby.clone(),
                );

                *lobby_id.lock().await = Some(generated_lobby_id);

                loop {
                    let msg = match rx.try_next().await? {
                        Some(tungstenite::Message::Binary(d)) => {
                            tango_protos::lobby::CreateStreamToServerMessage::decode(
                                bytes::Bytes::from(d),
                            )?
                        }
                        Some(tungstenite::Message::Close(_)) | None => {
                            return Ok(());
                        }
                        Some(m) => {
                            anyhow::bail!("unexpected message: {:?}", m);
                        }
                    };

                    match msg {
                        tango_protos::lobby::CreateStreamToServerMessage {
                            which:
                                Some(tango_protos::lobby::create_stream_to_server_message::Which::AcceptReq(
                                    accept_req,
                                )),
                        } => {
                            let mut lobby = lobby.lock().await;
                            let pp = if let Some(pp) = lobby.pending_players.get(&accept_req.opponent_id) {
                                pp.clone()
                            } else {
                                // No such player, just continue.
                                continue;
                            };

                            let mut pp = pp.lock().await;

                            let session_id = generate_id();

                            lobby.creator_tx.send(tungstenite::Message::Binary(tango_protos::lobby::CreateStreamToClientMessage {
                                which:
                                    Some(tango_protos::lobby::create_stream_to_client_message::Which::AcceptResp(
                                        tango_protos::lobby::create_stream_to_client_message::AcceptResponse {
                                            session_id: session_id.clone(),
                                        }
                                    )),
                            }.encode_to_vec())).await?;


                            pp.tx.send(tungstenite::Message::Binary(tango_protos::lobby::JoinStreamToClientMessage {
                                which:
                                    Some(tango_protos::lobby::join_stream_to_client_message::Which::AcceptInd(
                                        tango_protos::lobby::join_stream_to_client_message::AcceptIndication {
                                            session_id,
                                        }
                                    )),
                            }.encode_to_vec())).await?;

                            if let Some(close_sender) = pp.close_sender.take() {
                                let _ = close_sender.send(());
                            }
                            break;
                        },

                        tango_protos::lobby::CreateStreamToServerMessage {
                            which:
                                Some(tango_protos::lobby::create_stream_to_server_message::Which::RejectReq(
                                    reject_req,
                                )),
                        } => {
                            let mut lobby = lobby.lock().await;
                            let pp = if let Some(pp) = lobby.pending_players.remove(&reject_req.opponent_id) {
                                pp
                            } else {
                                // No such player, just continue.
                                continue;
                            };

                            lobby.creator_tx.send(tungstenite::Message::Binary(tango_protos::lobby::CreateStreamToClientMessage {
                                which:
                                    Some(tango_protos::lobby::create_stream_to_client_message::Which::RejectResp(
                                        tango_protos::lobby::create_stream_to_client_message::RejectResponse { }
                                    )),
                            }.encode_to_vec())).await?;

                            let mut pp = pp.lock().await;
                            if let Some(close_sender) = pp.close_sender.take() {
                                let _ = close_sender.send(());
                            }
                        },
                        m => anyhow::bail!("unexpected message: {:?}", m),
                    };
                }

                Ok(())
            })().await
        };

        if let Some(lobby_id) = &*lobby_id.lock().await {
            let mut lobbies = self.lobbies.lock().await;
            let lobby = if let Some(lobby) = lobbies.remove(lobby_id) {
                lobby
            } else {
                return r;
            };

            let mut lobby = lobby.lock().await;
            for (_, pp) in &mut lobby.pending_players {
                // TODO: Inform client why they're being disconnected.
                let mut pp = pp.lock().await;
                if let Some(close_sender) = pp.close_sender.take() {
                    let _ = close_sender.send(());
                }
            }
        }

        r
    }

    pub async fn handle_join_stream(
        &self,
        ws: hyper_tungstenite::WebSocketStream<hyper::upgrade::Upgraded>,
    ) -> anyhow::Result<()> {
        let (mut tx, mut rx) = ws.split();
        let lobby_and_opponent_id = std::sync::Arc::new(tokio::sync::Mutex::new(None));

        let r = {
            let lobbies = self.lobbies.clone();
            let lobby_and_opponent_id = lobby_and_opponent_id.clone();

            (move || async move {
                let msg = match rx.try_next().await? {
                    Some(tungstenite::Message::Binary(d)) => {
                        tango_protos::lobby::JoinStreamToServerMessage::decode(bytes::Bytes::from(
                            d,
                        ))?
                    }
                    Some(tungstenite::Message::Close(_)) | None => {
                        return Ok(());
                    }
                    Some(m) => {
                        anyhow::bail!("unexpected message: {:?}", m);
                    }
                };

                let join_req = match msg {
                    tango_protos::lobby::JoinStreamToServerMessage {
                        which:
                            Some(tango_protos::lobby::join_stream_to_server_message::Which::JoinReq(
                                join_req,
                            )),
                    } => join_req,
                    m => anyhow::bail!("unexpected message: {:?}", m),
                };

                let game_info = if let Some(game_info) = join_req.game_info {
                    game_info
                } else {
                    anyhow::bail!("create request was missing game info");
                };

                let lobby = match lobbies.lock().await.get(&join_req.lobby_id) {
                    Some(lobby) => lobby.clone(),
                    None => {
                        anyhow::bail!("no such lobby");
                    }
                };

                let generated_opponent_id = generate_id();
                let (close_sender, close_receiver) = tokio::sync::oneshot::channel();
                {
                    let mut lobby = lobby.lock().await;

                    lobby.creator_tx.send(tungstenite::Message::Binary(tango_protos::lobby::CreateStreamToClientMessage {
                        which:
                            Some(tango_protos::lobby::create_stream_to_client_message::Which::JoinInd(
                                tango_protos::lobby::create_stream_to_client_message::JoinIndication {
                                    opponent_id: generated_opponent_id.clone(),
                                    game_info: Some(game_info),
                                    save_data: join_req.save_data,
                                }
                            )),
                    }.encode_to_vec())).await?;

                    tx.send(tungstenite::Message::Binary(tango_protos::lobby::JoinStreamToClientMessage {
                        which:
                            Some(tango_protos::lobby::join_stream_to_client_message::Which::JoinResp(
                                tango_protos::lobby::join_stream_to_client_message::JoinResponse {
                                    opponent_id: generated_opponent_id.clone(),
                                    game_info: Some(lobby.game_info.clone()),
                                    save_data: lobby.save_data.clone(),
                                }
                            )),
                    }.encode_to_vec())).await?;

                    let pp = std::sync::Arc::new(tokio::sync::Mutex::new(PendingPlayer {
                        tx,
                        close_sender: Some(close_sender),
                    }));
                    lobby
                        .pending_players
                        .insert(generated_opponent_id.clone(), pp);
                }

                *lobby_and_opponent_id.lock().await =
                    Some((join_req.lobby_id.clone(), generated_opponent_id.clone()));

                close_receiver.await?;

                Ok(())
            })()
            .await
        };

        if let Some((lobby_id, opponent_id)) = &*lobby_and_opponent_id.lock().await {
            let lobbies = self.lobbies.lock().await;
            let lobby = if let Some(lobby) = lobbies.get(lobby_id) {
                lobby
            } else {
                return r;
            };
            let mut lobby = lobby.lock().await;
            lobby.pending_players.remove(opponent_id);
        }

        r
    }
}