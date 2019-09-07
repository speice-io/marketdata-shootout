use std::convert::TryInto;

use nom::{bytes::complete::take, number::complete::*, sequence::tuple, IResult};

use crate::parsers::{extract_iex_data, read_block, Block};

pub struct IexParser<'a> {
    pcap_buffer: &'a [u8],
}

impl<'a> IexParser<'a> {
    pub fn new(pcap_buffer: &[u8]) -> IexParser {
        IexParser { pcap_buffer }
    }
}

impl<'a> Iterator for IexParser<'a> {
    type Item = IexPayload;

    fn next(&mut self) -> Option<Self::Item> {
        let mut buffer = self.pcap_buffer;
        while let Ok((rem, block)) = read_block(buffer) {
            self.pcap_buffer = rem;
            buffer = rem;
            match block {
                Block::EnhancedPacket(e) => {
                    let (_, iex_data) = extract_iex_data(e.packet_data).unwrap();
                    let (_, payload) = IexPayload::parse(iex_data).unwrap();
                    return Some(payload);
                }
                _ => (),
            }
        }

        None
    }
}

#[derive(Debug)]
pub struct IexPayload {
    version: u8,
    _reserved: u8,
    proto_id: u16,
    channel_id: u32,
    session_id: u32,
    payload_len: u16,
    pub msg_count: u16,
    stream_offset: u64,
    pub first_seq_no: u64,
    send_time: i64,
    pub messages: smallvec::SmallVec<[IexMessage; 256]>,
}

impl IexPayload {
    pub fn parse(payload: &[u8]) -> IResult<&[u8], IexPayload> {
        let (
            mut rem,
            (
                version,
                _reserved,
                proto_id,
                channel_id,
                session_id,
                payload_len,
                msg_count,
                stream_offset,
                first_seq_no,
                send_time,
            ),
        ) = tuple((
            le_u8, le_u8, le_u16, le_u32, le_u32, le_u16, le_u16, le_u64, le_u64, le_i64,
        ))(payload)?;

        let mut messages = smallvec::SmallVec::new();
        for _i in 0..msg_count {
            let (_rem, msg) = IexMessage::parse(rem)?;
            rem = _rem;
            messages.push(msg);
        }

        debug_assert!(rem.len() == 0);
        Ok((
            rem,
            IexPayload {
                version,
                _reserved,
                proto_id,
                channel_id,
                session_id,
                payload_len,
                msg_count,
                stream_offset,
                first_seq_no,
                send_time,
                messages,
            },
        ))
    }
}

#[derive(Debug)]
pub enum IexMessage {
    SystemEvent(SystemEvent),
    SecurityDirectory(SecurityDirectory),
    TradingStatus(TradingStatus),
    OperationalHaltStatus(OperationalHaltStatus),
    ShortSalePriceTest(ShortSalePriceTest),
    SecurityEvent(SecurityEvent),
    PriceLevelUpdate(PriceLevelUpdate),
    TradeReport(TradeReport),
    OfficialPrice(OfficialPrice),
    TradeBreak(TradeBreak),
    AuctionInformation(AuctionInformation),
}

macro_rules! parse_msg {
    ($input:ident, $len:ident, $msg_type:ident) => {{
        let (_, msg) = $msg_type::parse($input)?;
        IexMessage::$msg_type(msg)
    }};
}

impl IexMessage {
    // TODO: Benchmark a version where we cast a packed struct instead of parsing
    pub fn parse(input: &[u8]) -> IResult<&[u8], IexMessage> {
        let (payload, msg_len) = le_u16(input)?;

        // Minor technical note: IEX's docs state that the message format
        // can grow at any time, and we should always trust the msg_len field,
        // so we first slice off the total msg_len for future compatibility
        let (rem, payload) = take(msg_len)(payload)?;
        let msg = match payload[0] {
            0x53 => parse_msg!(payload, msg_len, SystemEvent),
            0x44 => parse_msg!(payload, msg_len, SecurityDirectory),
            0x48 => parse_msg!(payload, msg_len, TradingStatus),
            0x4f => parse_msg!(payload, msg_len, OperationalHaltStatus),
            0x50 => parse_msg!(payload, msg_len, ShortSalePriceTest),
            0x45 => parse_msg!(payload, msg_len, SecurityEvent),
            // Why the "match multiple" looks like bitwise-OR is beyond me.
            0x38 | 0x35 => parse_msg!(payload, msg_len, PriceLevelUpdate),
            0x54 => parse_msg!(payload, msg_len, TradeReport),
            0x58 => parse_msg!(payload, msg_len, OfficialPrice),
            0x42 => parse_msg!(payload, msg_len, TradeBreak),
            0x41 => parse_msg!(payload, msg_len, AuctionInformation),
            t => panic!("Unrecognized message type={}, payload={:?}", t, payload),
        };

        Ok((rem, msg))
    }
}

#[derive(Debug)]
pub struct SystemEvent {
    msg_type: u8,
    system_event: u8,
    timestamp: i64,
}

impl SystemEvent {
    fn parse(input: &[u8]) -> IResult<&[u8], SystemEvent> {
        let (rem, (msg_type, system_event, timestamp)) = tuple((le_u8, le_u8, le_i64))(input)?;

        Ok((
            rem,
            SystemEvent {
                msg_type,
                system_event,
                timestamp,
            },
        ))
    }
}

#[derive(Debug)]
pub struct SecurityDirectory {
    msg_type: u8,
    flags: u8,
    timestamp: i64,
    symbol: [u8; 8],
    lot_size: u32,
    previous_closing: u64,
    luld_tier: u8,
}

impl SecurityDirectory {
    fn parse(input: &[u8]) -> IResult<&[u8], SecurityDirectory> {
        let (rem, (msg_type, flags, timestamp, symbol, lot_size, previous_closing, luld_tier)) =
            tuple((le_u8, le_u8, le_i64, take(8usize), le_u32, le_u64, le_u8))(input)?;

        Ok((
            rem,
            SecurityDirectory {
                msg_type,
                flags,
                timestamp,
                symbol: symbol.try_into().unwrap(),
                lot_size,
                previous_closing,
                luld_tier,
            },
        ))
    }
}

#[derive(Debug)]
pub struct TradingStatus {
    msg_type: u8,
    trading_status: u8,
    timestamp: i64,
    symbol: [u8; 8],
    reason: [u8; 4],
}

impl TradingStatus {
    fn parse(input: &[u8]) -> IResult<&[u8], TradingStatus> {
        let (rem, (msg_type, trading_status, timestamp, symbol, reason)) =
            tuple((le_u8, le_u8, le_i64, take(8usize), take(4usize)))(input)?;

        Ok((
            rem,
            TradingStatus {
                msg_type,
                trading_status,
                timestamp,
                symbol: symbol.try_into().unwrap(),
                reason: reason.try_into().unwrap(),
            },
        ))
    }
}

#[derive(Debug)]
pub struct OperationalHaltStatus {
    msg_type: u8,
    halt_status: u8,
    timestamp: i64,
    symbol: [u8; 8],
}

impl OperationalHaltStatus {
    fn parse(input: &[u8]) -> IResult<&[u8], OperationalHaltStatus> {
        let (rem, (msg_type, halt_status, timestamp, symbol)) =
            tuple((le_u8, le_u8, le_i64, take(8usize)))(input)?;

        Ok((
            rem,
            OperationalHaltStatus {
                msg_type,
                halt_status,
                timestamp,
                symbol: symbol.try_into().unwrap(),
            },
        ))
    }
}

#[derive(Debug)]
pub struct ShortSalePriceTest {
    msg_type: u8,
    sspt_status: u8,
    timestamp: i64,
    symbol: [u8; 8],
    detail: u8,
}

impl ShortSalePriceTest {
    fn parse(input: &[u8]) -> IResult<&[u8], ShortSalePriceTest> {
        let (rem, (msg_type, sspt_status, timestamp, symbol, detail)) =
            tuple((le_u8, le_u8, le_i64, take(8usize), le_u8))(input)?;

        Ok((
            rem,
            ShortSalePriceTest {
                msg_type,
                sspt_status,
                timestamp,
                symbol: symbol.try_into().unwrap(),
                detail,
            },
        ))
    }
}

#[derive(Debug)]
pub struct SecurityEvent {
    msg_type: u8,
    security_event: u8,
    timestamp: i64,
    symbol: [u8; 8],
}

impl SecurityEvent {
    fn parse(input: &[u8]) -> IResult<&[u8], SecurityEvent> {
        let (rem, (msg_type, security_event, timestamp, symbol)) =
            tuple((le_u8, le_u8, le_i64, take(8usize)))(input)?;

        Ok((
            rem,
            SecurityEvent {
                msg_type,
                security_event,
                timestamp,
                symbol: symbol.try_into().unwrap(),
            },
        ))
    }
}

#[derive(Debug)]
pub struct PriceLevelUpdate {
    pub msg_type: u8,
    pub event_flags: u8,
    pub timestamp: i64,
    pub symbol: [u8; 8],
    pub size: u32,
    pub price: u64,
}

impl PriceLevelUpdate {
    fn parse(input: &[u8]) -> IResult<&[u8], PriceLevelUpdate> {
        let (rem, (msg_type, event_flags, timestamp, symbol, size, price)) =
            tuple((le_u8, le_u8, le_i64, take(8usize), le_u32, le_u64))(input)?;

        Ok((
            rem,
            PriceLevelUpdate {
                msg_type,
                event_flags,
                timestamp,
                symbol: symbol.try_into().unwrap(),
                size,
                price,
            },
        ))
    }
}

#[derive(Debug)]
pub struct TradeReport {
    pub msg_type: u8,
    pub sale_condition: u8,
    pub timestamp: i64,
    pub symbol: [u8; 8],
    pub size: u32,
    pub price: u64,
    pub trade_id: u64,
}

impl TradeReport {
    fn parse(input: &[u8]) -> IResult<&[u8], TradeReport> {
        let (rem, (msg_type, sale_condition, timestamp, symbol, size, price, trade_id)) =
            tuple((le_u8, le_u8, le_i64, take(8usize), le_u32, le_u64, le_u64))(input)?;

        Ok((
            rem,
            TradeReport {
                msg_type,
                sale_condition,
                timestamp,
                symbol: symbol.try_into().unwrap(),
                size,
                price,
                trade_id,
            },
        ))
    }
}

#[derive(Debug)]
pub struct OfficialPrice {
    msg_type: u8,
    price_type: u8,
    timestamp: i64,
    symbol: [u8; 8],
    official_price: u64,
}

impl OfficialPrice {
    fn parse(input: &[u8]) -> IResult<&[u8], OfficialPrice> {
        let (rem, (msg_type, price_type, timestamp, symbol, official_price)) =
            tuple((le_u8, le_u8, le_i64, take(8usize), le_u64))(input)?;

        Ok((
            rem,
            OfficialPrice {
                msg_type,
                price_type,
                timestamp,
                symbol: symbol.try_into().unwrap(),
                official_price,
            },
        ))
    }
}

#[derive(Debug)]
pub struct TradeBreak {
    msg_type: u8,
    sale_condition: u8,
    timestamp: i64,
    symbol: [u8; 8],
    size: u32,
    price: u64,
    trade_id: u64,
}

impl TradeBreak {
    fn parse(input: &[u8]) -> IResult<&[u8], TradeBreak> {
        let (rem, (msg_type, sale_condition, timestamp, symbol, size, price, trade_id)) =
            tuple((le_u8, le_u8, le_i64, take(8usize), le_u32, le_u64, le_u64))(input)?;

        Ok((
            rem,
            TradeBreak {
                msg_type,
                sale_condition,
                timestamp,
                symbol: symbol.try_into().unwrap(),
                size,
                price,
                trade_id,
            },
        ))
    }
}

#[derive(Debug)]
pub struct AuctionInformation {
    msg_type: u8,
    auction_type: u8,
    timestamp: i64,
    symbol: [u8; 8],
    paired_shares: u32,
    reference_price: u64,
    indicative_clearing_price: u64,
    imbalance_shares: u32,
    imbalance_side: u8,
    extension_number: u8,
    scheduled_auction: u32,
    auction_book_clearing_price: u64,
    collar_reference_price: u64,
    lower_auction_collar: u64,
    upper_auction_collar: u64,
}

impl AuctionInformation {
    fn parse(input: &[u8]) -> IResult<&[u8], AuctionInformation> {
        // Dear Lord, why?
        let (
            rem,
            (
                msg_type,
                auction_type,
                timestamp,
                symbol,
                paired_shares,
                reference_price,
                indicative_clearing_price,
                imbalance_shares,
                imbalance_side,
                extension_number,
                scheduled_auction,
                auction_book_clearing_price,
                collar_reference_price,
                lower_auction_collar,
                upper_auction_collar,
            ),
        ) = tuple((
            le_u8,
            le_u8,
            le_i64,
            take(8usize),
            le_u32,
            le_u64,
            le_u64,
            le_u32,
            le_u8,
            le_u8,
            le_u32,
            le_u64,
            le_u64,
            le_u64,
            le_u64,
        ))(input)?;

        Ok((
            rem,
            AuctionInformation {
                msg_type,
                auction_type,
                timestamp,
                symbol: symbol.try_into().unwrap(),
                paired_shares,
                reference_price,
                indicative_clearing_price,
                imbalance_shares,
                imbalance_side,
                extension_number,
                scheduled_auction,
                auction_book_clearing_price,
                collar_reference_price,
                lower_auction_collar,
                upper_auction_collar,
            },
        ))
    }
}
