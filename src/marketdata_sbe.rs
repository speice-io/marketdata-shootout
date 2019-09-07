/// Generated code for SBE package marketdata_sbe

/// Imports core rather than std to broaden usable environments.
extern crate core;

/// Result types for error handling

/// Errors that may occur during the course of encoding or decoding.
#[derive(Debug)]
pub enum CodecErr {
    /// Too few bytes in the byte-slice to read or write the data structure relevant
    /// to the current state of the codec
    NotEnoughBytes,

    /// Groups and vardata are constrained by the numeric type chosen to represent their
    /// length as well as optional maxima imposed by the schema
    SliceIsLongerThanAllowedBySchema,
}

pub type CodecResult<T> = core::result::Result<T, CodecErr>;

/// Scratch Decoder Data Wrapper - codec internal use only
#[derive(Debug)]
pub struct ScratchDecoderData<'d> {
    data: &'d [u8],
    pos: usize,
}

impl<'d> ScratchDecoderData<'d> {
    /// Create a struct reference overlaid atop the data buffer
    /// such that the struct's contents directly reflect the buffer.
    /// Advances the `pos` index by the size of the struct in bytes.
    #[inline]
    fn read_type<T>(&mut self, num_bytes: usize) -> CodecResult<&'d T> {
        let end = self.pos + num_bytes;
        if end <= self.data.len() {
            let s = self.data[self.pos..end].as_ptr() as *mut T;
            let v: &'d T = unsafe { &*s };
            self.pos = end;
            Ok(v)
        } else {
            Err(CodecErr::NotEnoughBytes)
        }
    }

    /// Advances the `pos` index by a set number of bytes.
    #[inline]
    fn skip_bytes(&mut self, num_bytes: usize) -> CodecResult<()> {
        let end = self.pos + num_bytes;
        if end <= self.data.len() {
            self.pos = end;
            Ok(())
        } else {
            Err(CodecErr::NotEnoughBytes)
        }
    }

    /// Create a slice reference overlaid atop the data buffer
    /// such that the slice's members' contents directly reflect the buffer.
    /// Advances the `pos` index by the size of the slice contents in bytes.
    #[inline]
    fn read_slice<T>(&mut self, count: usize, bytes_per_item: usize) -> CodecResult<&'d [T]> {
        let num_bytes = bytes_per_item * count;
        let end = self.pos + num_bytes;
        if end <= self.data.len() {
            let v: &'d [T] = unsafe {
                core::slice::from_raw_parts(self.data[self.pos..end].as_ptr() as *const T, count)
            };
            self.pos = end;
            Ok(v)
        } else {
            Err(CodecErr::NotEnoughBytes)
        }
    }
}

/// Scratch Encoder Data Wrapper - codec internal use only
#[derive(Debug)]
pub struct ScratchEncoderData<'d> {
    data: &'d mut [u8],
    pos: usize,
}

impl<'d> ScratchEncoderData<'d> {
    /// Copy the bytes of a value into the data buffer
    /// Advances the `pos` index to after the newly-written bytes.
    #[inline]
    fn write_type<T>(&mut self, t: &T, num_bytes: usize) -> CodecResult<()> {
        let end = self.pos + num_bytes;
        if end <= self.data.len() {
            let source_bytes: &[u8] =
                unsafe { core::slice::from_raw_parts(t as *const T as *const u8, num_bytes) };
            (&mut self.data[self.pos..end]).copy_from_slice(source_bytes);
            self.pos = end;
            Ok(())
        } else {
            Err(CodecErr::NotEnoughBytes)
        }
    }

    /// Advances the `pos` index by a set number of bytes.
    #[inline]
    fn skip_bytes(&mut self, num_bytes: usize) -> CodecResult<()> {
        let end = self.pos + num_bytes;
        if end <= self.data.len() {
            self.pos = end;
            Ok(())
        } else {
            Err(CodecErr::NotEnoughBytes)
        }
    }

    /// Create a struct reference overlaid atop the data buffer
    /// such that changes to the struct directly edit the buffer.
    /// Note that the initial content of the struct's fields may be garbage.
    /// Advances the `pos` index to after the newly-written bytes.
    #[inline]
    fn writable_overlay<T>(&mut self, num_bytes: usize) -> CodecResult<&'d mut T> {
        let end = self.pos + num_bytes;
        if end <= self.data.len() {
            let v: &'d mut T = unsafe {
                let s = self.data.as_ptr().offset(self.pos as isize) as *mut T;
                &mut *s
            };
            self.pos = end;
            Ok(v)
        } else {
            Err(CodecErr::NotEnoughBytes)
        }
    }

    /// Copy the bytes of a value into the data buffer at a specific position
    /// Does **not** alter the `pos` index.
    #[inline]
    fn write_at_position<T>(
        &mut self,
        position: usize,
        t: &T,
        num_bytes: usize,
    ) -> CodecResult<()> {
        let end = position + num_bytes;
        if end <= self.data.len() {
            let source_bytes: &[u8] =
                unsafe { core::slice::from_raw_parts(t as *const T as *const u8, num_bytes) };
            (&mut self.data[position..end]).copy_from_slice(source_bytes);
            Ok(())
        } else {
            Err(CodecErr::NotEnoughBytes)
        }
    }
    /// Create a mutable slice overlaid atop the data buffer directly
    /// such that changes to the slice contents directly edit the buffer
    /// Note that the initial content of the slice's members' fields may be garbage.
    /// Advances the `pos` index to after the region representing the slice.
    #[inline]
    fn writable_slice<T>(
        &mut self,
        count: usize,
        bytes_per_item: usize,
    ) -> CodecResult<&'d mut [T]> {
        let end = self.pos + (count * bytes_per_item);
        if end <= self.data.len() {
            let v: &'d mut [T] = unsafe {
                core::slice::from_raw_parts_mut(
                    self.data[self.pos..end].as_mut_ptr() as *mut T,
                    count,
                )
            };
            self.pos = end;
            Ok(v)
        } else {
            Err(CodecErr::NotEnoughBytes)
        }
    }

    /// Copy the raw bytes of a slice's contents into the data buffer
    /// Does **not** encode the length of the slice explicitly into the buffer.
    /// Advances the `pos` index to after the newly-written slice bytes.
    #[inline]
    fn write_slice_without_count<T>(&mut self, t: &[T], bytes_per_item: usize) -> CodecResult<()> {
        let content_bytes_size = bytes_per_item * t.len();
        let end = self.pos + content_bytes_size;
        if end <= self.data.len() {
            let source_bytes: &[u8] =
                unsafe { core::slice::from_raw_parts(t.as_ptr() as *const u8, content_bytes_size) };
            (&mut self.data[self.pos..end]).copy_from_slice(source_bytes);
            self.pos = end;
            Ok(())
        } else {
            Err(CodecErr::NotEnoughBytes)
        }
    }
}

/// Convenience Either enum
#[derive(Copy, Clone, PartialEq, Eq, PartialOrd, Ord, Hash, Debug)]
pub enum Either<L, R> {
    Left(L),
    Right(R),
}

/// Enum Side
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
#[repr(u8)]
pub enum Side {
    Buy = 0u8,
    Sell = 1u8,
    NullVal = 255u8,
}
impl Default for Side {
    fn default() -> Self {
        Side::NullVal
    }
}

/// Enum MsgType
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
#[repr(u8)]
pub enum MsgType {
    Trade = 0u8,
    Quote = 1u8,
    NullVal = 255u8,
}
impl Default for MsgType {
    fn default() -> Self {
        MsgType::NullVal
    }
}

/// Quote
#[repr(C, packed)]
#[derive(Default)]
pub struct Quote {
    pub price: u64,
    pub size: u32,
    pub flags: u8,
    pub side: Side,
}

impl Quote {}

/// Trade
#[repr(C, packed)]
#[derive(Default)]
pub struct Trade {
    pub price: u64,
    pub size: u32,
}

impl Trade {}

/// MessageHeader
#[repr(C, packed)]
#[derive(Default)]
pub struct MessageHeader {
    pub block_length: u16,
    pub template_id: u16,
    pub schema_id: u16,
    pub version: u16,
}

impl MessageHeader {}

/// GroupSizeEncoding
#[repr(C, packed)]
#[derive(Default)]
pub struct GroupSizeEncoding {
    pub block_length: u16,
    pub num_in_group: u16,
}

impl GroupSizeEncoding {}

/// VarAsciiEncoding
#[repr(C, packed)]
#[derive(Default)]
pub struct VarAsciiEncoding {
    pub length: u32,
    pub var_data: u8,
}

impl VarAsciiEncoding {}

/// MessageHeader Decoder entry point
pub fn start_decoding_message_header<'d>(
    data: &'d [u8],
) -> CodecResult<(&'d MessageHeader, ScratchDecoderData<'d>)> {
    let mut scratch = ScratchDecoderData { data: data, pos: 0 };
    let v = scratch.read_type::<MessageHeader>(8)?;
    Ok((v, scratch))
}

/// MultiMessage Fixed-size Fields (8 bytes)
#[repr(C, packed)]
#[derive(Default)]
pub struct MultiMessageFields {
    pub sequence_number: u64,
}

impl MultiMessageFields {}

/// MultiMessage specific Message Header
#[repr(C, packed)]
pub struct MultiMessageMessageHeader {
    pub message_header: MessageHeader,
}
impl MultiMessageMessageHeader {
    pub const BLOCK_LENGTH: u16 = 8;
    pub const TEMPLATE_ID: u16 = 1;
    pub const SCHEMA_ID: u16 = 1;
    pub const VERSION: u16 = 0;
}
impl Default for MultiMessageMessageHeader {
    fn default() -> MultiMessageMessageHeader {
        MultiMessageMessageHeader {
            message_header: MessageHeader {
                block_length: 8u16,
                template_id: 1u16,
                schema_id: 1u16,
                version: 0u16,
            },
        }
    }
}

/// Group fixed-field member representations
#[repr(C, packed)]
#[derive(Default)]
pub struct MultiMessageMessagesMember {
    pub timestamp: i64,
    pub msg_type: MsgType,
    pub trade: Trade,
    pub quote: Quote,
}

impl MultiMessageMessagesMember {}

/// MultiMessageDecoderDone
pub struct MultiMessageDecoderDone<'d> {
    scratch: ScratchDecoderData<'d>,
}
impl<'d> MultiMessageDecoderDone<'d> {
    /// Returns the number of bytes decoded
    pub fn unwrap(self) -> usize {
        self.scratch.pos
    }

    pub fn wrap(scratch: ScratchDecoderData<'d>) -> MultiMessageDecoderDone<'d> {
        MultiMessageDecoderDone { scratch: scratch }
    }
}

/// symbol variable-length data
pub struct MultiMessageMessagesSymbolDecoder<'d> {
    parent: MultiMessageMessagesMemberDecoder<'d>,
}
impl<'d> MultiMessageMessagesSymbolDecoder<'d> {
    fn wrap(parent: MultiMessageMessagesMemberDecoder<'d>) -> Self {
        MultiMessageMessagesSymbolDecoder { parent: parent }
    }
    pub fn symbol(
        mut self,
    ) -> CodecResult<(
        &'d [u8],
        Either<MultiMessageMessagesMemberDecoder<'d>, MultiMessageDecoderDone<'d>>,
    )> {
        let count = *self.parent.scratch.read_type::<u32>(4)?;
        Ok((
            self.parent.scratch.read_slice::<u8>(count as usize, 1)?,
            self.parent.after_member(),
        ))
    }
}

/// MultiMessageMessages Decoder for fields and header
pub struct MultiMessageMessagesMemberDecoder<'d> {
    scratch: ScratchDecoderData<'d>,
    max_index: u16,
    index: u16,
}

impl<'d> MultiMessageMessagesMemberDecoder<'d> {
    fn new(scratch: ScratchDecoderData<'d>, count: u16) -> Self {
        assert!(count > 0u16);
        MultiMessageMessagesMemberDecoder {
            scratch: scratch,
            max_index: count - 1,
            index: 0,
        }
    }

    pub fn next_messages_member(
        mut self,
    ) -> CodecResult<(
        &'d MultiMessageMessagesMember,
        MultiMessageMessagesSymbolDecoder<'d>,
    )> {
        let v = self.scratch.read_type::<MultiMessageMessagesMember>(35)?;
        self.index += 1;
        Ok((v, MultiMessageMessagesSymbolDecoder::wrap(self)))
    }
    #[inline]
    fn after_member(
        self,
    ) -> Either<MultiMessageMessagesMemberDecoder<'d>, MultiMessageDecoderDone<'d>> {
        if self.index <= self.max_index {
            Either::Left(self)
        } else {
            Either::Right(MultiMessageDecoderDone::wrap(self.scratch))
        }
    }
}
pub struct MultiMessageMessagesHeaderDecoder<'d> {
    scratch: ScratchDecoderData<'d>,
}
impl<'d> MultiMessageMessagesHeaderDecoder<'d> {
    fn wrap(scratch: ScratchDecoderData<'d>) -> Self {
        MultiMessageMessagesHeaderDecoder { scratch: scratch }
    }
    pub fn messages_individually(
        mut self,
    ) -> CodecResult<Either<MultiMessageMessagesMemberDecoder<'d>, MultiMessageDecoderDone<'d>>>
    {
        let dim = self.scratch.read_type::<GroupSizeEncoding>(4)?;
        if dim.num_in_group > 0 {
            Ok(Either::Left(MultiMessageMessagesMemberDecoder::new(
                self.scratch,
                dim.num_in_group,
            )))
        } else {
            Ok(Either::Right(MultiMessageDecoderDone::wrap(self.scratch)))
        }
    }
}

/// MultiMessage Fixed fields Decoder
pub struct MultiMessageFieldsDecoder<'d> {
    scratch: ScratchDecoderData<'d>,
}
impl<'d> MultiMessageFieldsDecoder<'d> {
    pub fn wrap(scratch: ScratchDecoderData<'d>) -> MultiMessageFieldsDecoder<'d> {
        MultiMessageFieldsDecoder { scratch: scratch }
    }
    pub fn multi_message_fields(
        mut self,
    ) -> CodecResult<(
        &'d MultiMessageFields,
        MultiMessageMessagesHeaderDecoder<'d>,
    )> {
        let v = self.scratch.read_type::<MultiMessageFields>(8)?;
        Ok((v, MultiMessageMessagesHeaderDecoder::wrap(self.scratch)))
    }
}

/// MultiMessageMessageHeaderDecoder
pub struct MultiMessageMessageHeaderDecoder<'d> {
    scratch: ScratchDecoderData<'d>,
}
impl<'d> MultiMessageMessageHeaderDecoder<'d> {
    pub fn wrap(scratch: ScratchDecoderData<'d>) -> MultiMessageMessageHeaderDecoder<'d> {
        MultiMessageMessageHeaderDecoder { scratch: scratch }
    }
    pub fn header(mut self) -> CodecResult<(&'d MessageHeader, MultiMessageFieldsDecoder<'d>)> {
        let v = self.scratch.read_type::<MessageHeader>(8)?;
        Ok((v, MultiMessageFieldsDecoder::wrap(self.scratch)))
    }
}

/// MultiMessage Decoder entry point
pub fn start_decoding_multi_message<'d>(data: &'d [u8]) -> MultiMessageMessageHeaderDecoder<'d> {
    MultiMessageMessageHeaderDecoder::wrap(ScratchDecoderData { data: data, pos: 0 })
}

/// MultiMessageEncoderDone
pub struct MultiMessageEncoderDone<'d> {
    scratch: ScratchEncoderData<'d>,
}
impl<'d> MultiMessageEncoderDone<'d> {
    /// Returns the number of bytes encoded
    pub fn unwrap(self) -> usize {
        self.scratch.pos
    }

    pub fn wrap(scratch: ScratchEncoderData<'d>) -> MultiMessageEncoderDone<'d> {
        MultiMessageEncoderDone { scratch: scratch }
    }
}

/// symbol variable-length data
pub struct MultiMessageMessagesSymbolEncoder<'d> {
    parent: MultiMessageMessagesMemberEncoder<'d>,
}
impl<'d> MultiMessageMessagesSymbolEncoder<'d> {
    fn wrap(parent: MultiMessageMessagesMemberEncoder<'d>) -> Self {
        MultiMessageMessagesSymbolEncoder { parent: parent }
    }
    pub fn symbol(mut self, s: &'d [u8]) -> CodecResult<MultiMessageMessagesMemberEncoder> {
        let l = s.len();
        if l > 4294967294 {
            return Err(CodecErr::SliceIsLongerThanAllowedBySchema);
        }
        // Write data length
        self.parent.scratch.write_type::<u32>(&(l as u32), 4)?; // group length
        self.parent.scratch.write_slice_without_count::<u8>(s, 1)?;
        Ok(self.parent)
    }
}

/// MultiMessageMessages Encoder for fields and header
pub struct MultiMessageMessagesMemberEncoder<'d> {
    scratch: ScratchEncoderData<'d>,
    count_write_pos: usize,
    count: u16,
}

impl<'d> MultiMessageMessagesMemberEncoder<'d> {
    #[inline]
    fn new(scratch: ScratchEncoderData<'d>, count_write_pos: usize) -> Self {
        MultiMessageMessagesMemberEncoder {
            scratch: scratch,
            count_write_pos: count_write_pos,
            count: 0,
        }
    }

    #[inline]
    pub fn next_messages_member(
        mut self,
        fields: &MultiMessageMessagesMember,
    ) -> CodecResult<MultiMessageMessagesSymbolEncoder<'d>> {
        self.scratch
            .write_type::<MultiMessageMessagesMember>(fields, 35)?; // block length
        self.count += 1;
        Ok(MultiMessageMessagesSymbolEncoder::wrap(self))
    }
    #[inline]
    pub fn done_with_messages(mut self) -> CodecResult<MultiMessageEncoderDone<'d>> {
        self.scratch
            .write_at_position::<u16>(self.count_write_pos, &self.count, 2)?;
        Ok(MultiMessageEncoderDone::wrap(self.scratch))
    }
}
pub struct MultiMessageMessagesHeaderEncoder<'d> {
    scratch: ScratchEncoderData<'d>,
}
impl<'d> MultiMessageMessagesHeaderEncoder<'d> {
    #[inline]
    fn wrap(scratch: ScratchEncoderData<'d>) -> Self {
        MultiMessageMessagesHeaderEncoder { scratch: scratch }
    }
    #[inline]
    pub fn messages_individually(mut self) -> CodecResult<MultiMessageMessagesMemberEncoder<'d>> {
        self.scratch.write_type::<u16>(&35u16, 2)?; // block length
        let count_pos = self.scratch.pos;
        self.scratch.write_type::<u16>(&0, 2)?; // preliminary group member count
        Ok(MultiMessageMessagesMemberEncoder::new(
            self.scratch,
            count_pos,
        ))
    }
}

/// MultiMessage Fixed fields Encoder
pub struct MultiMessageFieldsEncoder<'d> {
    scratch: ScratchEncoderData<'d>,
}
impl<'d> MultiMessageFieldsEncoder<'d> {
    pub fn wrap(scratch: ScratchEncoderData<'d>) -> MultiMessageFieldsEncoder<'d> {
        MultiMessageFieldsEncoder { scratch: scratch }
    }

    /// Create a mutable struct reference overlaid atop the data buffer
    /// such that changes to the struct directly edit the buffer.
    /// Note that the initial content of the struct's fields may be garbage.
    pub fn multi_message_fields(
        mut self,
    ) -> CodecResult<(
        &'d mut MultiMessageFields,
        MultiMessageMessagesHeaderEncoder<'d>,
    )> {
        let v = self.scratch.writable_overlay::<MultiMessageFields>(8 + 0)?;
        Ok((v, MultiMessageMessagesHeaderEncoder::wrap(self.scratch)))
    }

    /// Copy the bytes of a value into the data buffer
    pub fn multi_message_fields_copy(
        mut self,
        t: &MultiMessageFields,
    ) -> CodecResult<MultiMessageMessagesHeaderEncoder<'d>> {
        self.scratch.write_type::<MultiMessageFields>(t, 8)?;
        Ok(MultiMessageMessagesHeaderEncoder::wrap(self.scratch))
    }
}

/// MultiMessageMessageHeaderEncoder
pub struct MultiMessageMessageHeaderEncoder<'d> {
    scratch: ScratchEncoderData<'d>,
}
impl<'d> MultiMessageMessageHeaderEncoder<'d> {
    pub fn wrap(scratch: ScratchEncoderData<'d>) -> MultiMessageMessageHeaderEncoder<'d> {
        MultiMessageMessageHeaderEncoder { scratch: scratch }
    }

    /// Create a mutable struct reference overlaid atop the data buffer
    /// such that changes to the struct directly edit the buffer.
    /// Note that the initial content of the struct's fields may be garbage.
    pub fn header(mut self) -> CodecResult<(&'d mut MessageHeader, MultiMessageFieldsEncoder<'d>)> {
        let v = self.scratch.writable_overlay::<MessageHeader>(8 + 0)?;
        Ok((v, MultiMessageFieldsEncoder::wrap(self.scratch)))
    }

    /// Copy the bytes of a value into the data buffer
    pub fn header_copy(mut self, t: &MessageHeader) -> CodecResult<MultiMessageFieldsEncoder<'d>> {
        self.scratch.write_type::<MessageHeader>(t, 8)?;
        Ok(MultiMessageFieldsEncoder::wrap(self.scratch))
    }
}

/// MultiMessage Encoder entry point
pub fn start_encoding_multi_message<'d>(
    data: &'d mut [u8],
) -> MultiMessageMessageHeaderEncoder<'d> {
    MultiMessageMessageHeaderEncoder::wrap(ScratchEncoderData { data: data, pos: 0 })
}
