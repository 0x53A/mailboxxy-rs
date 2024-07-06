# Mailboxxy
#### a small, F# and Erlang inspired Micro Actor library


I have never used a "big" Actory System like Akka or similar. I have used and enjoyed the [``MailboxProcessor``](https://fsharp.github.io/fsharp-core-docs/reference/fsharp-control-fsharpmailboxprocessor-1.html) in F#, which is an extremely lightweight actor implementation.

In short, it allows to create _agents_, which internally run *_synchronously_* (which massively simplifies their implementation), but run asynchronously and parallel with each other and to the main thread.

This pattern is, in my experience, really useful if you have a stateful resource which you share with multiple callers.

Instead of the callers having to synchronize access to the resource, they asynchronously dispatch messages to the resource, which handles them in a loop and optionally responds with an answer.

The programming interface against the actor might look a little bit unusual, instead of an Interface (in .NET) / a Trait (in Rust), you define a discriminated union with all possible _calls_ as union cases. A return message is declared by a case member of type ``ReplyChannel<T>``.

I believe the sample code will make this clearer:

A dead simple "counter" actor, which can count up (by 1), count down (by a specified value), or can be queried for its current value.

```rust
enum CounterMsg {
    Increment,
    Decrement(i32),
    GetValue(ReplyChannel<i32>)
}
```

If you have a reference to the actor

```rust
let mb : Mailbox<CounterMsg> = // ...
```

then you can post messages with the two functions ``post``and ``ask``. Both of these will asynchronously dispatch a message, ``ask`` will return a ``Future`` with a return value:

```rust
    mb.post(CounterMsg::Increment);
    mb.post(CounterMsg::Increment);
    mb.post(CounterMsg::Increment);
    let val = mb.ask(|rc| CounterMsg::GetValue(rc)).await;
    assert_eq!(val, 3);

    mb.post(CounterMsg::Decrement(1));
    let val = mb.ask(|rc| CounterMsg::GetValue(rc)).await;
    assert_eq!(val, 2);
```

The incredible strength (in my opinion) of this pattern is that in a process with many threads, you could have a number of references to this single Counter actor, and in each location, you can post messages to it without having to worry about synchronization or ownership. The actor will synchronize everything internally.

Of course, if a different thread was posting Increment or Decrement messages in parallel to the code block above, GetValue might return a different value than 3 and 2, respectively, depending on what was called.

With how to program against an actor out of the way, how do I implement the actor itself?

Easy, as a simple loop which reads a message and handles it:

```rust
// define the actor function
async fn mailbox_fn(ctx:MailboxContext<CounterMsg>) {

    // local state
    let mut count = 0;

    loop {
        let msg: CounterMsg = ctx.dequeue().await;

        match msg {
            CounterMsg::Increment => count = count + 1,
            CounterMsg::Decrement(n) => count = count - n,
            CounterMsg::GetValue(rc) => rc.reply(count)
        }
    }
}

// start the actor
let mb = start_mailbox(mailbox_fn);
```

_(please note that this code is simplified and omits some parameters for brevity, look at the unit test in lib.rs for details)_


## License

dual licensed under MIT and CC0 (public domain)

## References

As mentioned at the top, this is heavily inspired by the F# MailboxProcess (which in turn was apparently inspired by Erlang).

Before implementing this myself, I searched for an existing micro actor framework amd found [a different MailboxProcessor port](https://github.com/garydwatson/mailbox_processor) by Gary Watson. The implementation by Gary Watson differes from the F# implementation (and mine) in that there's only one possible return value *per actor*, not per message case. It doesn't seem to have ported the concept of the ReplyChannel.

