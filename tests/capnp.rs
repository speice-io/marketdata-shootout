use alloc_counter::{AllocCounterSystem, count_alloc, deny_alloc};

use md_shootout::marketdata_capnp::multi_message;

#[global_allocator]
static A: AllocCounterSystem = AllocCounterSystem;

#[test]
fn reinit_memory_check() {
    // Setting up the builder doesn't reserve any heap memory
    let mut msg_block = deny_alloc(|| {
        capnp::message::Builder::new_default()
    });

    // Setting up the root object, however, does reserve a first segment
    let (stats, result) = count_alloc(|| {
        let multimsg = msg_block.init_root::<multi_message::Builder>();
        multimsg.init_messages(32);
    });

    assert_eq!(stats.0, 4);
    assert_eq!(stats.1, 0);
    assert_eq!(stats.2, 0);

    // If we reinitialize an object on that original builder, we re-use memory
    deny_alloc(|| {
        let multimsg = msg_block.init_root::<multi_message::Builder>();
        multimsg.init_messages(32);

        // Even if we down-size and up-size the message list size, we don't need
        // to re-allocate
        let multimsg = msg_block.init_root::<multi_message::Builder>();
        multimsg.init_messages(16);

        let multimsg = msg_block.init_root::<multi_message::Builder>();
        multimsg.init_messages(32);
    });

    // It's only when we init a larger message count that a fresh allocation occurs
    let (stats, _) = count_alloc(|| {
        let multimsg = msg_block.init_root::<multi_message::Builder>();
        // Note: calling `init_messages(33)` doesn't force allocation because
        // the Capnproto builder reserved extra memory the first time around
        multimsg.init_messages(256);
    });

    assert_eq!(stats.0, 1);
    assert_eq!(stats.1, 3);
    assert_eq!(stats.2, 0);
}