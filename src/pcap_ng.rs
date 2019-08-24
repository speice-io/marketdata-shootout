use std::mem::size_of;

use nom::{
    branch::alt,
    bytes::complete::tag,
    bytes::complete::take,
    IResult,
    number::complete::*,
    sequence::tuple,
};

use crate::pcap_ng::Block::EnhancedPacket;

pub enum Block<'a> {
    SectionHeader(SectionHeaderBlock),
    InterfaceDescription(InterfaceDescriptionBlock),
    EnhancedPacket(EnhancedPacketBlock<'a>),
}

pub fn read_block(input: &[u8]) -> IResult<&[u8], Block> {
    alt((
        section_header_block,
        interface_description_block,
        enhanced_packet_block
    ))(input)
}

#[derive(Debug)]
pub struct SectionHeaderBlock {
    block_len: u32
}

const SECTION_HEADER: [u8; 4] = [0x0a, 0x0d, 0x0d, 0x0a];

pub fn section_header_block(input: &[u8]) -> IResult<&[u8], Block> {
    let header_len = 12;
    let (rem, (_, block_len, _)) = tuple((
        tag(SECTION_HEADER),
        le_u32,
        tag([0x4d, 0x3c, 0x2b, 0x1a])
    ))(input)?;

    take(block_len - header_len)(rem)
        .map(|i| (i.0, Block::SectionHeader(SectionHeaderBlock {
            block_len
        })))
}

#[derive(Debug)]
pub struct InterfaceDescriptionBlock {
    block_len: u32
}

const INTERFACE_DESCRIPTION: [u8; 4] = [0x01, 0x00, 0x00, 0x00];

pub fn interface_description_block(input: &[u8]) -> IResult<&[u8], Block> {
    let header_len = 8;
    let (rem, (_, block_len)) = tuple((
        tag(INTERFACE_DESCRIPTION),
        le_u32
    ))(input)?;

    take(block_len - header_len)(rem)
        .map(|i| (i.0, Block::InterfaceDescription(InterfaceDescriptionBlock {
            block_len
        })))
}

pub struct EnhancedPacketBlock<'a> {
    pub block_len: u32,
    pub packet_data: &'a [u8],
}

const ENHANCED_PACKET: [u8; 4] = [0x06, 0x00, 0x00, 0x00];

pub fn enhanced_packet_block(input: &[u8]) -> IResult<&[u8], Block> {
    let header_len = 28;
    let (rem, (_, block_len, _, _, _, captured_len, _)) = tuple((
        tag(ENHANCED_PACKET),
        le_u32,
        le_u32,
        le_u32,
        le_u32,
        le_u32,
        le_u32
    ))(input)?;

    let (rem, packet_data) = take(captured_len)(rem)?;

    // Packets are supposed to be padded to 32 bits, but IEX DEEP doesn't
    // seem to respect this
    //let packet_total_len = (captured_len + 3) / 4 * 4;

    take(block_len - header_len - captured_len)(rem)
        .map(|i| (i.0, Block::EnhancedPacket(EnhancedPacketBlock {
            block_len,
            packet_data,
        })))
}