use crate::print;
use core::{
    pin::Pin,
    task::{Context, Poll},
};

use crate::println;

use conquer_once::spin::OnceCell;
use crossbeam_queue::ArrayQueue;
use futures_util::{task::AtomicWaker, Stream, StreamExt as _};
use pc_keyboard::{layouts, DecodedKey, HandleControl, Keyboard, ScancodeSet1};

static SCANCODE_QUEUE: OnceCell<ArrayQueue<u8>> = OnceCell::uninit();
static WAKER: AtomicWaker = AtomicWaker::new();

pub(crate) fn add_scancode(scancode: u8) {
    if let Ok(queue) = SCANCODE_QUEUE.try_get() {
        if let Err(_) = queue.push(scancode) {
            println!("WARNING: scancode queue full; dropping keyboard input");
        } else {
            WAKER.wake();
        }
    } else {
        println!("WARNING: keyboard scancode queue uninitialized");
    }
}

// The `_private` field is here to make the `new` function the only way to create the struct
pub struct ScancodeStream {
    _private: (),
}

impl ScancodeStream {
    pub fn new() -> Self {
        SCANCODE_QUEUE
            .try_init_once(|| ArrayQueue::new(100))
            .expect("ScancodeStream::new should only be called once!");
        ScancodeStream { _private: () }
    }
}

impl Stream for ScancodeStream {
    type Item = u8;

    fn poll_next(self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Option<Self::Item>> {
        let queue = SCANCODE_QUEUE
            .try_get()
            .expect("scancode stream is not initialized");

        // to avoid registering the waker if the queue isn't empty
        if let Some(c) = queue.pop() {
            return Poll::Ready(Some(c));
        }

        WAKER.register(&cx.waker());
        match queue.pop() {
            Some(c) => {
                WAKER.take();
                Poll::Ready(Some(c))
            }
            None => Poll::Pending,
        }
    }
}

pub async fn print_keypresses() {
    let mut scancodes = ScancodeStream::new();
    let mut keyboard = Keyboard::new(layouts::Us104Key, ScancodeSet1, HandleControl::Ignore);

    while let Some(scancode) = scancodes.next().await {
        if let Ok(Some(keyevent)) = keyboard.add_byte(scancode) {
            if let Some(key) = keyboard.process_keyevent(keyevent) {
                match key {
                    DecodedKey::Unicode(c) => print!("{}", c),
                    DecodedKey::RawKey(key) => print!("{:?}", key),
                }
            }
        }
    }
}

pub async fn process_keypresses(mut key_press_handler: impl FnMut(DecodedKey)) {
    let mut scancodes = ScancodeStream::new();
    let mut keyboard = Keyboard::new(layouts::Us104Key, ScancodeSet1, HandleControl::Ignore);

    while let Some(scancode) = scancodes.next().await {
        if let Ok(Some(keyevent)) = keyboard.add_byte(scancode) {
            if let Some(key) = keyboard.process_keyevent(keyevent) {
                key_press_handler(key);
            }
        }
    }
}
