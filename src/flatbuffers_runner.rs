use std::convert::TryInto;
use std::io::{BufRead, Write};
use std::mem::size_of;

use crate::iex::{IexMessage, IexPayload};
use crate::marketdata_generated::md_shootout;
use crate::{RunnerDeserialize, RunnerSerialize, StreamVec, Summarizer};

pub struct FlatbuffersWriter<'a> {
    builder: flatbuffers::FlatBufferBuilder<'a>,
    message_buffer: Vec<flatbuffers::WIPOffset<md_shootout::Message<'a>>>,
}

impl<'a> FlatbuffersWriter<'a> {
    pub fn new() -> FlatbuffersWriter<'static> {
        FlatbuffersWriter {
            builder: flatbuffers::FlatBufferBuilder::new(),
            message_buffer: Vec::new(),
        }
    }
}

impl<'a> RunnerSerialize for FlatbuffersWriter<'a> {
    fn serialize(&mut self, payload: &IexPayload, output: &mut Vec<u8>) {
        // Because FlatBuffers can't handle nested vectors (specifically, we can't track
        // both the variable-length vector of messages, and the variable-length strings
        // within those messages), we have to cache the messages as they get built
        // so they can be added all at once later.

        for iex_msg in &payload.messages {
            let msg_args = match iex_msg {
                IexMessage::TradeReport(tr) => {
                    // The `Args` objects used are wrappers over an underlying `Builder`.
                    // We trust release builds to optimize out the wrapper.
                    let trade = md_shootout::Trade::create(
                        &mut self.builder,
                        &md_shootout::TradeArgs {
                            price: tr.price,
                            size_: tr.size,
                        },
                    );

                    let sym_str = self.builder.create_string(crate::parse_symbol(&tr.symbol));
                    Some(md_shootout::MessageArgs {
                        ts_nanos: tr.timestamp,
                        symbol: Some(sym_str),
                        body_type: md_shootout::MessageBody::Trade,
                        body: Some(trade.as_union_value()),
                    })
                }
                IexMessage::PriceLevelUpdate(plu) => {
                    let level_update = md_shootout::LevelUpdate::create(
                        &mut self.builder,
                        &md_shootout::LevelUpdateArgs {
                            price: plu.price,
                            size_: plu.size,
                            flags: plu.event_flags,
                            side: if plu.msg_type == 0x38 {
                                md_shootout::Side::Buy
                            } else {
                                md_shootout::Side::Sell
                            },
                        },
                    );

                    let sym_str = self.builder.create_string(crate::parse_symbol(&plu.symbol));
                    Some(md_shootout::MessageArgs {
                        ts_nanos: plu.timestamp,
                        symbol: Some(sym_str),
                        body_type: md_shootout::MessageBody::LevelUpdate,
                        body: Some(level_update.as_union_value()),
                    })
                }
                _ => None,
            };

            msg_args.map(|a| {
                let msg = md_shootout::Message::create(&mut self.builder, &a);
                self.message_buffer.push(msg);
            });
        }

        let messages = self.builder.create_vector(&self.message_buffer[..]);

        // Now that we've finished building all the messages, time to set up the final buffer
        let mut multimsg_builder = md_shootout::MultiMessageBuilder::new(&mut self.builder);
        multimsg_builder.add_seq_no(payload.first_seq_no);
        multimsg_builder.add_messages(messages);
        let multimsg = multimsg_builder.finish();

        // IMPORTANT NOTE: If you just `finish`, Flatbuffers has no idea where
        // an object ends in memory. To support streaming reads, you *must*
        // use `finish_size_prefixed`. This adds a LE u32 to the front of the payload.
        self.builder.finish_size_prefixed(multimsg, None);
        output.write(self.builder.finished_data()).unwrap();

        self.builder.reset();
        self.message_buffer.clear();
    }
}

pub struct FlatbuffersReader;

impl FlatbuffersReader {
    pub fn new() -> FlatbuffersReader {
        FlatbuffersReader {}
    }
}

impl RunnerDeserialize for FlatbuffersReader {
    fn deserialize<'a>(&self, buf: &'a mut StreamVec, stats: &mut Summarizer) -> Result<(), ()> {
        // Flatbuffers has kinda ad-hoc support for streaming: https://github.com/google/flatbuffers/issues/3898
        // Essentially, you can write an optional `u32` value to the front of each message
        // (`finish_size_prefixed` above) to figure out how long that message actually is.
        // Ultimately, end-users are responsible for all buffer management, "reading" is just
        // a view over the underlying buffer.
        let data = buf.fill_buf().unwrap();
        if data.len() == 0 {
            return Err(());
        }

        let msg_len_buf: [u8; 4] = data[..size_of::<u32>()].try_into().unwrap();
        let msg_len = u32::from_le_bytes(msg_len_buf) as usize;

        let multimsg = flatbuffers::get_size_prefixed_root::<md_shootout::MultiMessage>(data);
        let msg_vec = match multimsg.messages() {
            Some(m) => m,
            None => panic!("Couldn't find messages"),
        };

        for i in 0..msg_vec.len() {
            let msg: md_shootout::Message = msg_vec.get(i);
            match msg.body_type() {
                md_shootout::MessageBody::Trade => {
                    let trade = msg.body_as_trade().unwrap();
                    stats.append_trade_volume(msg.symbol().unwrap(), trade.size_().into());
                }
                md_shootout::MessageBody::LevelUpdate => {
                    let lu = msg.body_as_level_update().unwrap();
                    let is_bid = match lu.side() {
                        md_shootout::Side::Buy => true,
                        _ => false,
                    };
                    stats.update_quote_prices(msg.symbol().unwrap(), lu.price(), is_bid);
                }
                md_shootout::MessageBody::NONE => panic!("Unrecognized message type"),
            }
        }

        buf.consume(msg_len + size_of::<u32>());
        Ok(())
    }
}
