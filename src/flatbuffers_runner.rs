use std::io::Write;
use std::str::from_utf8_unchecked;

use capnp::data::new_builder;
use flatbuffers::buffer_has_identifier;
use nom::{bytes::complete::take_until, IResult};

use crate::iex::{IexMessage, IexPayload};
use crate::marketdata_generated::md_shootout;

fn __take_until<'a>(tag: &'static str, input: &'a [u8]) -> IResult<&'a [u8], &'a [u8]> {
    take_until(tag)(input)
}

fn parse_symbol(sym: &[u8; 8]) -> &str {
    // TODO: Use the `jetscii` library for all that SIMD goodness
    // IEX guarantees ASCII, so we're fine using an unsafe conversion
    let (_, sym_bytes) = __take_until(" ", &sym[..]).unwrap();
    unsafe { from_utf8_unchecked(sym_bytes) }
}

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

    pub fn serialize(&mut self, payload: &IexPayload, output: &mut Vec<u8>) {

        // Because FlatBuffers can't handle nested vectors (specifically, we can't track
        // both the variable-length vector of messages, and the variable-length strings
        // within those messages), we have to cache the messages as they get built
        // so they can be added all at once later.

        for iex_msg in &payload.messages {
            let msg_args = match iex_msg {
                IexMessage::TradeReport(tr) => {
                    // The `Args` objects used are wrappers over an underlying `Builder`.
                    // We trust release builds to optimize out the wrapper, but would be
                    // interesting to know whether that's actually the case.
                    let trade = md_shootout::Trade::create(
                        &mut self.builder,
                        &md_shootout::TradeArgs {
                            price: tr.price,
                            size_: tr.size,
                        },
                    );

                    /*
                    let mut trade_builder = md_shootout::TradeBuilder::new(self.builder);
                    trade_builder.add_price(tr.price);
                    trade_builder.add_size_(tr.size);
                    let trade = trade_builder.finish();
                    */
                    let sym_str = self.builder.create_string(parse_symbol(&tr.symbol));
                    Some(md_shootout::MessageArgs {
                        ts_nanos: tr.timestamp,
                        symbol: Some(sym_str),
                        body_type: md_shootout::MessageBody::Trade,
                        // Why the hell do I need the `as_union_value` function to convert to UnionWIPOffset???
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
                            side: if plu.msg_type == 0x38 { md_shootout::Side::Buy } else { md_shootout::Side::Sell },
                        },
                    );

                    let sym_str = self.builder.create_string(parse_symbol(&plu.symbol));
                    Some(md_shootout::MessageArgs {
                        ts_nanos: plu.timestamp,
                        symbol: Some(sym_str),
                        body_type: md_shootout::MessageBody::LevelUpdate,
                        body: Some(level_update.as_union_value()),
                    })
                }
                _ => None
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
        self.builder.finish(multimsg, None);
        ;
        output.write(self.builder.finished_data());

        self.builder.reset();
        self.message_buffer.clear();
    }
}
