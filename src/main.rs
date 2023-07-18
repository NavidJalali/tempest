use std::io::{StdoutLock, Write};

use anyhow::{bail, Context};
use serde::{Deserialize, Serialize};
use serde_json::Deserializer;

#[derive(Serialize, Deserialize, Debug, Clone)]
struct Message {
    #[serde(rename = "src")]
    source: String,
    #[serde(rename = "dest")]
    destination: String,
    body: Body,
}

#[derive(Serialize, Deserialize, Debug, Clone)]
struct Body {
    #[serde(rename = "msg_id")]
    id: Option<usize>,
    in_reply_to: Option<usize>,

    #[serde(flatten)]
    payload: Payload,
}

#[derive(Serialize, Deserialize, Debug, Clone)]
#[serde(tag = "type")]
#[serde(rename_all = "snake_case")]
enum Payload {
    Init {
        node_id: String,
        node_ids: Vec<String>,
    },
    InitOk {},

    Echo {
        echo: String,
    },
    EchoOk {
        echo: String,
    },
}

#[derive(Debug)]
enum EchoNodeState {
    Initializing,
    Ready { self_id: String, message_id: usize },
}

struct EchoNode {
    state: EchoNodeState,
}

impl EchoNode {
    fn next(&mut self, message: Message, output: &mut StdoutLock) -> anyhow::Result<()> {
        match &mut self.state {
            EchoNodeState::Initializing => {
                let payload = message.body.payload;

                match payload {
                    Payload::Echo { .. } => {
                        // Not in ready state. Ignore.
                    }

                    Payload::Init {
                        node_id,
                        node_ids: _,
                    } => {
                        self.state = EchoNodeState::Ready {
                            self_id: node_id.into(),
                            message_id: 1,
                        };
                        let reply = Message {
                            source: message.destination,
                            destination: message.source,
                            body: Body {
                                id: Some(0),
                                in_reply_to: message.body.id,
                                payload: Payload::InitOk {},
                            },
                        };

                        serde_json::to_writer(&mut *output, &reply)
                            .context("Could not encode maelstrom output.")?;

                        output.write_all(b"\n").context("New Line")?;
                    }

                    _ => (),
                }
            }

            EchoNodeState::Ready {
                self_id,
                message_id,
            } => {
                let payload = message.body.payload;

                if message.destination == self_id.as_ref() {
                    match payload {
                        Payload::Echo { echo } => {
                            let reply = Message {
                                source: message.destination,
                                destination: message.source,
                                body: Body {
                                    id: Some(message_id.clone()),
                                    in_reply_to: message.body.id,
                                    payload: Payload::EchoOk { echo },
                                },
                            };

                            serde_json::to_writer(&mut *output, &reply)
                                .context("Could not encode maelstrom output.")?;

                            output.write_all(b"\n").context("New Line")?;

                            self.state = EchoNodeState::Ready {
                                self_id: self_id.clone(),
                                message_id: *message_id + 1,
                            };
                        }

                        Payload::Init { .. } => {
                            // Already in ready state. Fail!
                            bail!("Received Init message while in ready state.")
                        }

                        _ => (),
                    }
                } else {
                    bail!("Received message for another node.")
                }
            }
        }
        Ok(())
    }
}

fn main() -> anyhow::Result<()> {
    let stdin = std::io::stdin().lock();
    let mut stdout = std::io::stdout().lock();
    let inputs = Deserializer::from_reader(stdin).into_iter::<Message>();
    let mut node = EchoNode {
        state: EchoNodeState::Initializing,
    };

    for input in inputs {
        let input = input.context("Could not decode maelstrom input.")?;

        node.next(input, &mut stdout)?;
    }

    Ok(())
}
