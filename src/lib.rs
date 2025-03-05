use std::future::Future;

use async_std::channel::*;

#[cfg(test)] use async_std::task;


pub struct ReplyChannel<T> {
    s: Sender<T>,
}

impl<T> ReplyChannel<T> {
    pub fn reply(&self, value: T) {
        self.s.send_blocking(value).unwrap();
        self.s.close();
    }
}


pub struct MailBox<TMessage, THandle> {
    sender: Sender<TMessage>,
    pub handle: THandle
}


impl<TMessage, THandle> MailBox<TMessage, THandle> {
    pub fn post(&self, msg: TMessage) {
        self.sender.send_blocking(msg).unwrap();
    }

    pub async fn ask<TResult>(&self, cb: fn(ReplyChannel<TResult>) -> TMessage) -> TResult{

        let (s,r) = bounded(1);

        let rc = ReplyChannel { s };
        let msg = cb(rc);
        self.sender.send_blocking(msg).unwrap();
        return r.recv().await.unwrap();
    }
}


pub struct MailboxContext<TMessage> {
    receiver: Receiver<TMessage>
}

impl<TMessage> MailboxContext<TMessage> {
    pub async fn dequeue(&self) -> TMessage {
        return self.receiver.recv().await.unwrap();
    }
}

pub enum MailboxBounds {
    Unbounded,
    Bounded(usize)
}

pub fn start_mailbox<TMessage, F, Fut, THandle>(bounds: MailboxBounds, f: F, spawn: fn(Fut) -> THandle) -> MailBox<TMessage, THandle> 
where
    F : FnOnce(MailboxContext<TMessage>) -> Fut,
    Fut : Future<Output = ()>
{
    let (s,r) = match bounds {
        MailboxBounds::Unbounded => unbounded(),
        MailboxBounds::Bounded(n) => bounded(n)
    };

    let ctx = MailboxContext { receiver: r };

    let handle = spawn(f(ctx));

    return MailBox { sender: s, handle };
}


#[cfg(test)]
enum TestMsg {
    Increment,
    Decrement,
    GetValue(ReplyChannel<i32>)
}

#[cfg(test)]
async fn mailbox_fn(ctx:MailboxContext<TestMsg>) {

    // local state
    let mut count = 0;

    loop {
        let msg: TestMsg = ctx.dequeue().await;

        match msg {
            TestMsg::Increment => count = count + 1,
            TestMsg::Decrement => count = count - 1,
            TestMsg::GetValue(rc) => rc.reply(count)
        }
    }
}

#[cfg(test)]
async fn test_async() {

    let mb = start_mailbox(MailboxBounds::Unbounded, mailbox_fn, task::spawn);

    mb.post(TestMsg::Increment);
    mb.post(TestMsg::Increment);
    mb.post(TestMsg::Increment);
    let val = mb.ask(|rc| TestMsg::GetValue(rc)).await;
    assert_eq!(val, 3);

    mb.post(TestMsg::Decrement);
    let val = mb.ask(|rc| TestMsg::GetValue(rc)).await;
    assert_eq!(val, 2);
}

#[cfg(test)]
async fn test_thread() {
    let mb = start_mailbox(MailboxBounds::Unbounded, mailbox_fn, |fut| {
        std::thread::spawn(move || {
            smol::block_on(fut) // Run the future inside the thread
        })
    });

    mb.post(TestMsg::Increment);
    mb.post(TestMsg::Increment);
    mb.post(TestMsg::Increment);
    let val = mb.ask(|rc| TestMsg::GetValue(rc)).await;
    assert_eq!(val, 3);

    mb.post(TestMsg::Decrement);
    let val = mb.ask(|rc| TestMsg::GetValue(rc)).await;
    assert_eq!(val, 2);
}


#[test]
fn run_test_async() {
    smol::block_on(test_async());
}

#[test]
fn run_test_thread() {
    smol::block_on(test_thread());
}