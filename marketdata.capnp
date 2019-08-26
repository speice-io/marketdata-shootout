@0x9b88118c58e937dc;

struct MultiMessage {
    seqNo @0 :UInt64;
    messages @1 :List(Message);
}

struct Message {
    ts @0 :Int64;
    symbol @1 :Text;

    union {
        trade @2 :Trade;
        quote @3 :LevelUpdate;
    }
}

struct Trade {
    price @0 :UInt64;
    size @1 :UInt32;
}

struct LevelUpdate {
    price @0 :UInt64;
    size @1 :UInt32;
    flags @2 :UInt8;
    side @3 :Side;
}

enum Side {
    buy @0;
    sell @1;
}
