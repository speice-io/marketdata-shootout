use std::io::{BufRead, Write};
use std::str::from_utf8_unchecked;

use crate::iex::{IexMessage, IexPayload};
use crate::marketdata_sbe::{
    start_decoding_multi_message, start_encoding_multi_message, Either, MultiMessageMessageHeader,
    MultiMessageMessagesMember, MultiMessageMessagesMemberEncoder,
    MultiMessageMessagesSymbolEncoder, Side,
};
use crate::{marketdata_sbe, RunnerDeserialize, RunnerSerialize, StreamVec, Summarizer};

pub struct SBEWriter {
    /// Buffer to construct messages before copying. While SBE benefits
    /// from easily being able to create messages directly in output buffer,
    /// we'll construct in a scratch buffer and then copy to more fairly
    /// benchmark against Cap'n Proto and Flatbuffers.
    scratch_buffer: Vec<u8>,
    default_header: MultiMessageMessageHeader,
}

impl SBEWriter {
    pub fn new() -> SBEWriter {
        SBEWriter {
            // 8K scratch buffer is *way* more than necessary,
            // but we don't want to run into issues with not enough
            // data to encode messages
            scratch_buffer: vec![0; 1024 * 8],
            default_header: MultiMessageMessageHeader::default(),
        }
    }
}

impl RunnerSerialize for SBEWriter {
    fn serialize(&mut self, payload: &IexPayload, output: &mut Vec<u8>) {
        let (fields, encoder) = start_encoding_multi_message(&mut self.scratch_buffer[..])
            .header_copy(&self.default_header.message_header)
            .unwrap()
            .multi_message_fields()
            .unwrap();
        fields.sequence_number = payload.first_seq_no;

        let encoder = encoder.messages_individually().unwrap();
        let encoder: MultiMessageMessagesMemberEncoder =
            payload.messages.iter().fold(encoder, |enc, m| match m {
                IexMessage::TradeReport(tr) => {
                    let fields = MultiMessageMessagesMember {
                        msg_type: marketdata_sbe::MsgType::Trade,
                        timestamp: tr.timestamp,
                        trade: marketdata_sbe::Trade {
                            size: tr.size,
                            price: tr.price,
                        },
                        ..Default::default()
                    };
                    let sym_enc: MultiMessageMessagesSymbolEncoder =
                        enc.next_messages_member(&fields).unwrap();
                    sym_enc
                        .symbol(crate::parse_symbol(&tr.symbol).as_bytes())
                        .unwrap()
                }
                IexMessage::PriceLevelUpdate(plu) => {
                    let fields = MultiMessageMessagesMember {
                        msg_type: marketdata_sbe::MsgType::Quote,
                        timestamp: plu.timestamp,
                        quote: marketdata_sbe::Quote {
                            price: plu.price,
                            size: plu.size,
                            flags: plu.event_flags,
                            side: if plu.msg_type == 0x38 {
                                Side::Buy
                            } else {
                                Side::Sell
                            },
                        },
                        ..Default::default()
                    };
                    let sym_enc: MultiMessageMessagesSymbolEncoder =
                        enc.next_messages_member(&fields).unwrap();
                    sym_enc
                        .symbol(crate::parse_symbol(&plu.symbol).as_bytes())
                        .unwrap()
                }
                _ => enc,
            });

        let finished = encoder.done_with_messages().unwrap();
        let data_len = finished.unwrap();

        output.write(&self.scratch_buffer[..data_len]).unwrap();
    }
}

pub struct SBEReader;

impl SBEReader {
    pub fn new() -> SBEReader {
        SBEReader {}
    }
}

impl RunnerDeserialize for SBEReader {
    fn deserialize<'a>(&self, buf: &'a mut StreamVec, stats: &mut Summarizer) -> Result<(), ()> {
        let data = buf.fill_buf().unwrap();
        if data.len() == 0 {
            return Err(());
        }

        let (_header, decoder) = start_decoding_multi_message(data).header().unwrap();

        let (_fields, decoder) = decoder.multi_message_fields().unwrap();
        let mut msg_decoder = decoder.messages_individually().unwrap();
        while let Either::Left(msg) = msg_decoder {
            let (member, sym_dec) = msg.next_messages_member().unwrap();
            let (sym, next_msg_dec) = sym_dec.symbol().unwrap();
            match member.msg_type {
                marketdata_sbe::MsgType::Trade => stats.append_trade_volume(
                    unsafe { from_utf8_unchecked(sym) },
                    member.trade.size as u64,
                ),
                marketdata_sbe::MsgType::Quote => stats.update_quote_prices(
                    unsafe { from_utf8_unchecked(sym) },
                    member.quote.price,
                    match member.quote.side {
                        Side::Buy => true,
                        _ => false,
                    },
                ),
                _ => (),
            }
            msg_decoder = next_msg_dec;
        }

        // We now have a `Right`, which is a finished messages block
        let msg_decoder = match msg_decoder {
            Either::Right(r) => r,
            _ => panic!("Didn't parse all messages"),
        };

        // Interestingly enough, `buf.consume(msg_decoder.unwrap())` isn't OK,
        // presumably something to do with when *precisely* the drop of `self`
        // happens for `msg_decoder`. Leave it as two statments so that
        // Rust is able to prove our immutable borrow of `data` ends in time
        // to consume the buffer
        let msg_len = msg_decoder.unwrap();
        buf.consume(msg_len);
        Ok(())
    }
}
