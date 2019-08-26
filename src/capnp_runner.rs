use std::cmp::{max, min};
use std::collections::hash_map::{DefaultHasher, HashMap};
use std::hash::Hasher;
use std::io::{BufReader, Error, Read};
use std::str::from_utf8_unchecked;

use capnp::message::ReaderOptions;
use capnp::serialize::{read_message, write_message};
use capnp::serialize_packed::{read_message as read_message_packed, write_message as write_message_packed};
use nom::bytes::complete::take_until;
use nom::IResult;

use crate::iex::{IexMessage, IexParser};
use crate::marketdata_capnp::{multi_message, Side};
use crate::marketdata_capnp::message;
use crate::SummaryStats;

fn __take_until<'a>(tag: &'static str, input: &'a [u8]) -> IResult<&'a [u8], &'a [u8]> {
    take_until(tag)(input)
}

fn parse_symbol(sym: &[u8; 8]) -> &str {
    // IEX guarantees ASCII, so we're fine using an unsafe conversion
    let (_, sym_bytes) = __take_until(" ", &sym[..]).unwrap();
    unsafe { from_utf8_unchecked(sym_bytes) }
}

pub fn serialize_capnp(parser: IexParser, size_hint: usize, packed: bool) -> Vec<u8> {
    let write_fn = if packed { write_message_packed } else { write_message };

    // Because CapNProto builds messages in heap before serialization,
    // we'll reserve memory up front and should avoid alloc calls later
    let mut capnp_message = capnp::message::Builder::new_default();
    let multimsg = capnp_message.init_root::<multi_message::Builder>();
    multimsg.init_messages(256);

    // Allocate our output buffer
    let mut output: Vec<u8> = Vec::with_capacity(size_hint);

    // Now to the actual work
    for iex_msg in parser {
        // Find the messages we actually care about in this context
        let num_msgs = iex_msg.messages.iter().map(|m| {
            match m {
                IexMessage::TradeReport(_) | IexMessage::PriceLevelUpdate(_) => 1,
                _ => 0
            }
        }).fold(0, |sum, i| sum + i);

        if num_msgs == 0 {
            continue;
        }

        // And actually serialize the IEX payload to CapNProto format
        let mut multimsg = capnp_message.init_root::<multi_message::Builder>();
        multimsg.set_seq_no(iex_msg.first_seq_no);

        let mut messages = multimsg.init_messages(num_msgs as u32);
        let mut current_msg_no = 0;
        for iex_msg in iex_msg.messages {
            match iex_msg {
                IexMessage::TradeReport(tr) => {
                    let mut message = messages.reborrow().get(current_msg_no);
                    current_msg_no += 1;
                    message.set_ts(tr.timestamp);

                    let sym = parse_symbol(&tr.symbol);
                    message.reborrow().init_symbol(sym.len() as u32);
                    message.set_symbol(sym);

                    let mut msg_tr = message.init_trade();
                    msg_tr.set_size(tr.size);
                    msg_tr.set_price(tr.price);
                }
                IexMessage::PriceLevelUpdate(plu) => {
                    let mut message = messages.reborrow().get(current_msg_no);
                    current_msg_no += 1;
                    message.set_ts(plu.timestamp);

                    let sym = parse_symbol(&plu.symbol);
                    message.reborrow().init_symbol(sym.len() as u32);
                    message.set_symbol(sym);

                    let mut msg_plu = message.init_quote();
                    msg_plu.set_price(plu.price);
                    msg_plu.set_size(plu.size);
                    msg_plu.set_flags(plu.event_flags);
                    msg_plu.set_side(if plu.msg_type == 0x38 { Side::Buy } else { Side::Sell });
                }
                _ => ()
            }
        }

        write_fn(&mut output, &capnp_message).unwrap();
    }

    output
}

struct AdvancingVec<'a> {
    pos: usize,
    inner: &'a Vec<u8>,
}

impl<'a> Read for AdvancingVec<'a> {
    fn read(&mut self, buf: &mut [u8]) -> Result<usize, Error> {
        // TODO: There's *got* to be a better way to handle this
        let end = self.pos + buf.len();
        let end = if end > self.inner.len() { self.inner.len() } else { end };
        let read_size = end - self.pos;
        buf[..read_size].copy_from_slice(&self.inner[self.pos..end]);
        self.pos = end;

        Ok(read_size)
    }
}

pub fn read_capnp(buffer: &Vec<u8>, packed: bool) -> HashMap<u64, SummaryStats> {
    let read_fn = if packed { read_message_packed } else { read_message };
    let unbuffered = AdvancingVec {
        pos: 0,
        inner: buffer,
    };
    let mut buffered = BufReader::new(unbuffered);
    let read_opts = ReaderOptions::new();

    let mut stats = HashMap::new();

    while let Ok(msg) = read_fn(&mut buffered, read_opts) {
        let multimsg = msg.get_root::<multi_message::Reader>().unwrap();

        for msg in multimsg.get_messages().unwrap().iter() {
            // Hash the symbol name since we can't return a HashMap containing
            // string pointers as the keys
            let sym = msg.get_symbol().unwrap();
            let mut h = DefaultHasher::new();
            h.write(sym.as_bytes());
            let key = h.finish();

            let mut sym_stats = stats.entry(key)
                .or_insert(SummaryStats::new(sym));

            match msg.which() {
                Ok(message::Trade(tr)) => {
                    let tr = tr.unwrap();
                    sym_stats.trade_volume += tr.get_size() as u64;
                }
                Ok(message::Quote(q)) => {
                    let q = q.unwrap();
                    if q.get_side().unwrap() == Side::Buy {
                        sym_stats.bid_high = max(sym_stats.bid_high, q.get_price());
                        sym_stats.bid_low = min(sym_stats.bid_low, q.get_price());
                    } else {
                        sym_stats.ask_high = max(sym_stats.ask_high, q.get_price());
                        sym_stats.ask_low = min(sym_stats.ask_low, q.get_price());
                    }
                }
                _ => {
                    panic!("Unrecognized message type")
                }
            }
        }
    }

    stats
}
