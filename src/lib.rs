pub struct ReplyChannel<T> {
    s: async_std::channel::Sender<T>,
}

impl<T> ReplyChannel<T> {
    pub fn reply(&self, value: T) {
        self.s.send_blocking(value).unwrap();
        self.s.close();
    }
}

pub struct MailBox<TMessage, THandle> {
    sender: async_std::channel::Sender<TMessage>,
    pub handle: THandle,
}

impl<TMessage, THandle> MailBox<TMessage, THandle> {
    pub fn post(&self, msg: TMessage) {
        self.sender.send_blocking(msg).unwrap();
    }

    pub async fn ask<TResult>(&self, cb: fn(ReplyChannel<TResult>) -> TMessage) -> TResult {
        let (s, r) = async_std::channel::bounded(1);

        let rc = ReplyChannel { s };
        let msg = cb(rc);
        self.sender.send_blocking(msg).unwrap();
        return r.recv().await.unwrap();
    }
}

pub struct MailboxContext<TMessage> {
    receiver: async_std::channel::Receiver<TMessage>,
}

impl<TMessage> MailboxContext<TMessage> {
    pub async fn dequeue(&self) -> TMessage {
        return self.receiver.recv().await.unwrap();
    }
}

pub enum MailboxBounds {
    Unbounded,
    Bounded(usize),
}

/// starts a mailbox, where the caller needs to handle actually starting the async fn.
/// 
/// ```
/// # use mailboxxy::*;
/// # enum TestMsg { }
/// # async fn my_mailbox_fn(ctx: MailboxContext<TestMsg>) { }
/// start_mailbox_direct(MailboxBounds::Unbounded, |ctx| async_std::task::spawn(my_mailbox_fn(ctx)));
/// ```
pub fn start_mailbox_direct<TMessage, F, THandle>(
    bounds: MailboxBounds,
    f: F,
) -> MailBox<TMessage, THandle>
where
    F: FnOnce(MailboxContext<TMessage>) -> THandle,
{
    let (s, r) = match bounds {
        MailboxBounds::Unbounded => async_std::channel::unbounded(),
        MailboxBounds::Bounded(n) => async_std::channel::bounded(n),
    };

    let ctx = MailboxContext { receiver: r };

    let handle = f(ctx);

    MailBox { sender: s, handle }
}

/// starts a mailbox using a given executor. Examples can include 'async_std::task::spawn' or 'std::thread::spawn'.
pub fn start_mailbox<TMessage, F, Fut, THandle>(
    bounds: MailboxBounds,
    f: F,
    spawn: fn(Fut) -> THandle,
) -> MailBox<TMessage, THandle>
where
    F: FnOnce(MailboxContext<TMessage>) -> Fut,
{
    start_mailbox_direct(bounds, |ctx| spawn(f(ctx)))
}

#[cfg(feature = "thread")]
pub fn start_mailbox_on_thread<TMessage, F, Fut>(
    bounds: MailboxBounds,
    f: F,
) -> MailBox<TMessage, std::thread::JoinHandle<()>>
where
    F: std::marker::Send + 'static + FnOnce(MailboxContext<TMessage>) -> Fut,
    Fut: std::future::Future<Output = ()>,
    TMessage: std::marker::Send + 'static,
{
    start_mailbox_direct(bounds, |ctx| {
        std::thread::spawn(move || {
            let fut = f(ctx);
            async_std::task::block_on(fut)
        })
    })
}

pub fn start_mailbox_as_task<TMessage, F, Fut>(
    bounds: MailboxBounds,
    f: F,
) -> MailBox<TMessage, async_std::task::JoinHandle<()>>
where
    F: std::marker::Send + 'static + FnOnce(MailboxContext<TMessage>) -> Fut,
    Fut: std::future::Future<Output = ()> + std::marker::Send + 'static,
    TMessage: std::marker::Send + 'static,
{
    start_mailbox_direct(bounds, |ctx| async_std::task::spawn(f(ctx)))
}

// ------------------------------------------------------------------------

#[cfg(test)]
enum TestMsg {
    Increment,
    Decrement,
    GetValue(ReplyChannel<i32>),
}

#[cfg(test)]
async fn mailbox_fn(ctx: MailboxContext<TestMsg>) {
    // local state
    let mut count = 0;

    loop {
        let msg: TestMsg = ctx.dequeue().await;

        match msg {
            TestMsg::Increment => count += 1,
            TestMsg::Decrement => count -= 1,
            TestMsg::GetValue(rc) => rc.reply(count),
        }
    }
}

#[cfg(test)]
async fn test_mb<T>(mb: &MailBox<TestMsg, T>) {
    mb.post(TestMsg::Increment);
    mb.post(TestMsg::Increment);
    mb.post(TestMsg::Increment);
    let val = mb.ask(TestMsg::GetValue).await;
    assert_eq!(val, 3);

    mb.post(TestMsg::Decrement);
    let val = mb.ask(TestMsg::GetValue).await;
    assert_eq!(val, 2);
}

#[cfg(test)]
async fn test_async() {
    let mb = start_mailbox(MailboxBounds::Unbounded, mailbox_fn, async_std::task::spawn);
    test_mb(&mb).await;
}

#[test]
fn run_test_async() {
    smol::block_on(test_async());
}

// ----------

#[cfg(test)]
async fn test_thread() {
    let mb = start_mailbox_on_thread(MailboxBounds::Unbounded, mailbox_fn);
    test_mb(&mb).await;
}

#[test]
fn run_test_thread() {
    smol::block_on(test_thread());
}

// ----------

// This tests that a fn, which is NOT Send, can be excuted on the thread executor.
// (It can't be executed as a task, since that would require Send'ing it between threads on resumption points.)

#[cfg(test)]
mod tests {
    use super::*;

    use std::collections::HashMap;

    trait TTest {
        fn test(&self) -> i32;
    }

    async fn mailbox_fn(ctx: MailboxContext<TestMsg>) {
        // local state
        let mut count = 0;
        let box_: HashMap<i32, Box<dyn TTest>> = HashMap::new();

        loop {
            let msg: TestMsg = ctx.dequeue().await;

            match msg {
                TestMsg::Increment => count += 1,
                TestMsg::Decrement => count -= 1,
                TestMsg::GetValue(rc) => {
                    box_.get(&count);
                    rc.reply(count)
                }
            }
        }
    }

    #[cfg(test)]
    async fn test_thread() {
        let mb = start_mailbox_on_thread(MailboxBounds::Unbounded, mailbox_fn);
        test_mb(&mb).await;
    }

    #[test]
    fn run_test_thread() {
        smol::block_on(test_thread());
    }
}
