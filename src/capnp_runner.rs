use std::convert::TryInto;
use std::io::{BufRead, Read};
use std::mem::size_of;
use std::str::from_utf8_unchecked;

use capnp::Error;
use capnp::message::{Builder, ReaderOptions, ScratchSpace, ScratchSpaceHeapAllocator};
use capnp::serialize::write_message;
use capnp::serialize_packed::{read_message as read_message_packed, write_message as write_message_packed};
use nom::bytes::complete::take_until;
use nom::IResult;

use crate::{RunnerDeserialize, RunnerSerialize, StreamVec, Summarizer};
use crate::iex::{IexMessage, IexPayload};
use crate::marketdata_capnp::{multi_message, Side};
use crate::marketdata_capnp::message;

pub struct CapnpWriter<'a> {
    // We have to be very careful with how messages are built, as running
    // `init_root` and rebuilding will still accumulate garbage if using
    // the standard HeapAllocator.
    // https://github.com/capnproto/capnproto-rust/issues/111
    words: Vec<capnp::Word>,
    scratch: ScratchSpace<'a>,
    packed: bool,
}

impl<'a> CapnpWriter<'a> {
    pub fn new(packed: bool) -> CapnpWriter<'a> {
        // Cap'n'Proto words are 8 bytes, MTU is 1500 bytes, theoretically need only 188 words.
        // In practice, let's just make sure everything fits.
        let mut words = capnp::Word::allocate_zeroed_vec(1024);

        let mut scratch = ScratchSpace::new(unsafe {
            std::mem::transmute(&mut words[..])
        });

        CapnpWriter {
            words,
            scratch,
            packed
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
}

impl<'a> RunnerSerialize for CapnpWriter<'a> {
    fn serialize(&mut self, payload: &IexPayload, mut output: &mut Vec<u8>) {
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

        // This is the safe builder used for testing
        //let mut builder = capnp::message::Builder::new_default();
        //let mut multimsg = builder.init_root::<multi_message::Builder>();

        // This is the unsafe (but faster) version
        let mut builder = self.builder();
        let mut multimsg = builder.init_root::<multi_message::Builder>();

        multimsg.set_seq_no(payload.first_seq_no);

        let mut messages = multimsg.init_messages(num_msgs as u32);
        let mut current_msg_no = 0;
        for iex_msg in payload.messages.iter() {
            match iex_msg {
                IexMessage::TradeReport(tr) => {
                    let mut message = messages.reborrow().get(current_msg_no);
                    current_msg_no += 1;
                    message.set_ts(tr.timestamp);

                    let sym = crate::parse_symbol(&tr.symbol);
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

                    let sym = crate::parse_symbol(&plu.symbol);
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

        let write_fn = if self.packed { write_message_packed } else { write_message };

        write_fn(&mut output, &builder).unwrap();
    }
}

pub struct CapnpReader {
    read_opts: ReaderOptions,
    packed: bool
}

impl CapnpReader {
    pub fn new(packed: bool) -> CapnpReader {
        CapnpReader {
            read_opts: ReaderOptions::new(),
            packed
        }
    }
}

impl CapnpReader {
    fn deserialize_packed<'a>(&self, buf: &'a mut StreamVec, stats: &mut Summarizer) -> Result<(), ()> {
        // Because `capnp::serialize_packed::PackedRead` is hidden from us, packed reads
        // *have* to both allocate new segments every read, and copy the buffer into
        // those same segments, no ability to re-use allocated memory.
        let reader = read_message_packed(buf, self.read_opts)
            .map_err(|_| ())?;

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

    fn deserialize_unpacked(&self, buf: &mut StreamVec, stats: &mut Summarizer) -> Result<(), ()> {
        let mut data = buf.fill_buf().map_err(|_| ())?;
        if data.len() == 0 {
            return Err(());
        }

        let orig_data = data;
        let reader_opts = ReaderOptions::default();

        /*
        Read into `OwnedSegments`, which means we copy the entire message into a new Vec. Note that
        the `data` pointer is modified underneath us, can figure out the message length by
        checking the difference between where we started and what `data` is afterward.
        This is a trick you learn only by looking at the fuzzing test cases.

        let reader = capnp::serialize::read_message(&mut data, reader_opts)?;
        let bytes_consumed = orig_data.len() - data.len();
        */

        /*
        Read into `SliceSegments`, which allows us to re-use the underlying message storage,
        but still forces a Vec allocation for `offsets`. Also requires us to copy code from
        Cap'n'Proto because `SliceSegments` has private fields, and `read_segment_table`
        is private. And all this because `read_segment_from_words` has a length check
        that triggers an error if our buffer is too large. What the hell?
        There is no documentation on how to calculate `bytes_consumed` when parsing by hand
        that I could find, you just have to guess and check until you figure this one out.
        */
        let (num_words, offsets) = read_segment_table(&mut data, reader_opts)
            .map_err(|_| ())?;
        let words = unsafe { capnp::Word::bytes_to_words(data) };
        let reader = capnp::message::Reader::new(
            SliceSegments {
                words: words,
                segment_slices: offsets,
            },
            reader_opts,
        );
        let segment_table_bytes = orig_data.len() - data.len();
        let msg_bytes = num_words * size_of::<capnp::Word>();
        let bytes_consumed = segment_table_bytes + msg_bytes;

        let multimsg = reader.get_root::<multi_message::Reader>()
            .map_err(|_| ())?;
        for msg in multimsg.get_messages().map_err(|_| ())?.iter() {
            let sym = msg.get_symbol().map_err(|_| ())?;

            match msg.which().map_err(|_| ())? {
                message::Trade(trade) => {
                    let trade = trade.unwrap();
                    stats.append_trade_volume(sym, trade.get_size().into());
                },
                message::Quote(quote) => {
                    let quote = quote.unwrap();
                    let is_buy = match quote.get_side().unwrap() {
                        Side::Buy => true,
                        _ => false
                    };
                    stats.update_quote_prices(sym, quote.get_price(), is_buy);
                },
            }
        }

        buf.consume(bytes_consumed);
        Ok(())
    }
}

impl RunnerDeserialize for CapnpReader {
    fn deserialize<'a>(&self, buf: &'a mut StreamVec, stats: &mut Summarizer) -> Result<(), ()> {
        // While this is an extra branch per call, we're going to assume that the overhead
        // is essentially nil in practice
        if self.packed {
            self.deserialize_packed(buf, stats)
        } else {
            self.deserialize_unpacked(buf, stats)
        }
    }
}


pub struct SliceSegments<'a> {
    words: &'a [capnp::Word],
    segment_slices: Vec<(usize, usize)>,
}

impl<'a> capnp::message::ReaderSegments for SliceSegments<'a> {
    fn get_segment<'b>(&'b self, id: u32) -> Option<&'b [capnp::Word]> {
        if id < self.segment_slices.len() as u32 {
            let (a, b) = self.segment_slices[id as usize];
            Some(&self.words[a..b])
        } else {
            None
        }
    }

    fn len(&self) -> usize {
        self.segment_slices.len()
    }
}

fn read_segment_table<R>(read: &mut R,
                         options: capnp::message::ReaderOptions)
                         -> capnp::Result<(usize, Vec<(usize, usize)>)>
    where R: Read
{
    let mut buf: [u8; 8] = [0; 8];

    // read the first Word, which contains segment_count and the 1st segment length
    read.read_exact(&mut buf)?;
    let segment_count = u32::from_le_bytes(buf[0..4].try_into().unwrap()).wrapping_add(1) as usize;

    if segment_count >= 512 {
        return Err(Error::failed(format!("Too many segments: {}", segment_count)))
    } else if segment_count == 0 {
        return Err(Error::failed(format!("Too few segments: {}", segment_count)))
    }

    let mut segment_slices = Vec::with_capacity(segment_count);
    let mut total_words = u32::from_le_bytes(buf[4..8].try_into().unwrap()) as usize;
    segment_slices.push((0, total_words));

    if segment_count > 1 {
        if segment_count < 4 {
            read.read_exact(&mut buf)?;
            for idx in 0..(segment_count - 1) {
                let segment_len =
                    u32::from_le_bytes(buf[(idx * 4)..(idx + 1) * 4].try_into().unwrap()) as usize;

                segment_slices.push((total_words, total_words + segment_len));
                total_words += segment_len;
            }
        } else {
            let mut segment_sizes = vec![0u8; (segment_count & !1) * 4];
            read.read_exact(&mut segment_sizes[..])?;
            for idx in 0..(segment_count - 1) {
                let segment_len =
                    u32::from_le_bytes(segment_sizes[(idx * 4)..(idx + 1) * 4].try_into().unwrap()) as usize;

                segment_slices.push((total_words, total_words + segment_len));
                total_words += segment_len;
            }
        }
    }

    // Don't accept a message which the receiver couldn't possibly traverse without hitting the
    // traversal limit. Without this check, a malicious client could transmit a very large segment
    // size to make the receiver allocate excessive space and possibly crash.
    if total_words as u64 > options.traversal_limit_in_words {
        return Err(Error::failed(
            format!("Message has {} words, which is too large. To increase the limit on the \
             receiving end, see capnp::message::ReaderOptions.", total_words)))
    }

    Ok((total_words, segment_slices))
}
