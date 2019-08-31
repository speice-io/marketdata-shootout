use std::str::from_utf8_unchecked;

use capnp::Error;
use capnp::message::{Builder, ReaderOptions, ScratchSpace, ScratchSpaceHeapAllocator};
use capnp::serialize::write_message;
use capnp::serialize_packed::{read_message as read_message_packed, write_message as write_message_packed};
use nom::bytes::complete::take_until;
use nom::IResult;

use crate::{StreamVec, Summarizer};
use crate::iex::{IexMessage, IexPayload};
use crate::marketdata_capnp::{multi_message, Side};
use crate::marketdata_capnp::message;

fn __take_until<'a>(tag: &'static str, input: &'a [u8]) -> IResult<&'a [u8], &'a [u8]> {
    take_until(tag)(input)
}

fn parse_symbol(sym: &[u8; 8]) -> &str {
    // TODO: Use the `jetscii` library for all that SIMD goodness
    // IEX guarantees ASCII, so we're fine using an unsafe conversion
    let (_, sym_bytes) = __take_until(" ", &sym[..]).unwrap();
    unsafe { from_utf8_unchecked(sym_bytes) }
}

pub struct CapnpWriter<'a> {
    // We have to be very careful with how messages are built, as running
    // `init_root` and rebuilding will still accumulate garbage if using
    // the standard HeapAllocator.
    // https://github.com/capnproto/capnproto-rust/issues/111
    words: Vec<capnp::Word>,
    scratch: ScratchSpace<'a>,
}

impl<'a> CapnpWriter<'a> {
    pub fn new() -> CapnpWriter<'a> {
        // Cap'n'Proto words are 8 bytes, MTU is 1500 bytes, theoretically need only 188 words.
        // In practice, let's just make sure everything fits.
        let mut words = capnp::Word::allocate_zeroed_vec(1024);

        let mut scratch = ScratchSpace::new(unsafe {
            std::mem::transmute(&mut words[..])
        });

        CapnpWriter {
            words,
            scratch,
        }
    }

    fn builder(&mut self) -> capnp::message::Builder<ScratchSpaceHeapAllocator<'a, 'a>> {
        // Builders are only safe to use for serializing a single message. We can re-use the
        // backing memory (unsafe because now both `self` and the returned builder have a
        // mutable reference to `self.scratch), but Bad Things happen if we don't drop
        // in between serialization.
        capnp::message::Builder::new(ScratchSpaceHeapAllocator::new(unsafe {
            std::mem::transmute(&mut self.scratch)
        }))
    }

    pub fn serialize(&mut self, payload: &IexPayload, mut output: &mut Vec<u8>, packed: bool) {
        // First, count the messages we actually care about.
        let num_msgs = payload.messages.iter().map(|m| {
            match m {
                IexMessage::TradeReport(_) | IexMessage::PriceLevelUpdate(_) => 1,
                _ => 0
            }
        }).fold(0, |sum, i| sum + i);

        if num_msgs == 0 {
            return;
        }

        // And actually serialize the IEX payload to CapNProto format

        // This is the unsafe (but faster) version
        let mut builder = self.builder();
        let mut multimsg = builder.init_root::<multi_message::Builder>();

        // And the safe version used for testing
        //let mut builder = capnp::message::Builder::new_default();
        //let mut multimsg = builder.init_root::<multi_message::Builder>();

        multimsg.set_seq_no(payload.first_seq_no);

        let mut messages = multimsg.init_messages(num_msgs as u32);
        let mut current_msg_no = 0;
        for iex_msg in payload.messages.iter() {
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

        let write_fn = if packed { write_message_packed } else { write_message };

        write_fn(&mut output, &builder).unwrap();
    }
}

pub struct CapnpReader {
    read_opts: ReaderOptions
}

impl CapnpReader {
    pub fn new() -> CapnpReader {
        CapnpReader {
            read_opts: ReaderOptions::new()
        }
    }

    pub fn deserialize_packed<'a>(&self, buf: &'a mut StreamVec, stats: &mut Summarizer) -> Result<(), Error> {
        // Because `capnp::serialize_packed::PackedRead` is hidden from us, packed reads
        // *have* to both allocate new segments every read, and copy the buffer into
        // those same segments. Un-packed reading can use `SliceSegments` for true zero-copy
        let reader = read_message_packed(buf, self.read_opts)?;

        let multimsg = reader.get_root::<multi_message::Reader>().unwrap();
        for msg in multimsg.get_messages().unwrap().iter() {
            match msg.which() {
                Ok(message::Trade(tr)) => {
                    let tr = tr.unwrap();
                    stats.append_trade_volume(msg.get_symbol().unwrap(), tr.get_size() as u64);
                },
                Ok(message::Quote(q)) => {
                    let q = q.unwrap();
                    let is_bid = match q.get_side().unwrap() {
                        Side::Buy => true,
                        _ => false
                    };
                    stats.update_quote_prices(msg.get_symbol().unwrap(), q.get_price(), is_bid);
                },
                _ => panic!("Unrecognized message type!")
            }
        };
        Ok(())
    }
}
