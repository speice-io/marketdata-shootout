// This is hand-written code to act as proof-of-concept
// for a more complex schema generator. Theoretically, the
// schema file that would be used to generate the code below
// looks something like this (influenced by Kaitai Struct):
//
// seq:
//   - id: ts_nanos
//     type: u64
//   - id: symbol
//     type: str
//   - id: msg_type
//     type: u8
//     enum: msg_type
//   - id: msg_body
//     type:
//       switch-on: msg_type
//       cases:
//         msg_type::trade: trade
//         msg_type::level_update: level_update
// enums:
//   msg_type:
//     0: trade
//     1: level_update
//
//   side:
//     0: buy
//     1: sell
//
// types:
//   trade:
//     seq:
//       - id: price
//         type: u64
//       - id: size
//         type: u32
//   level_update:
//     seq:
//       - id: price
//         type: u64
//       - id: size
//         type: u32
//       - id: flags
//         type: u8
//       - id: side
//         type: u8
//         enum: side
