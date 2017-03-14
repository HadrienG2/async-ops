//! Asynchronous operation monitoring
//!
//! Picture yourself in a situation where: you want to delegate work to another
//! hardware or software entity, such as a coroutine, a thread pool, a GPU, an
//! IO device, or even a server over a network. You know that the work is going
//! to take some time, and you have other things to do meanwhile, so you would
//! rather not wait for its completion. But you would like a way to monitor the
//! progress of this work, know when it's done, manage errors...
//!
//! Sounds familiar? That's because such asynchronous operations are pervasive
//! in modern computing. We have hundreds of incompatible abstractions for
//! dealing with them, each with a subtly different interface and subtly
//! different implementation trade-offs. Typically, changing the implementation
//! of an asynchronous operation by moving it to another OS process or piece of
//! hardware means that you cannot use your favorite abstraction anymore and
//! must switch all of your code to another asynchronous operation monitoring
//! abstraction better suited for your new backend.
//!
//! But what if there was another way? What if we could have reasonably general
//! abstractions for representing and monitoring asynchronous operations, that
//! work in all asynchronous computing settings, with reasonably consistent
//! ergonomics and optimal performance for the task at hand?
//!
//! This crate is an attempt to make this dream come true.

extern crate triple_buffer;

mod executor;
mod multithread;
mod server;
mod status;

use multithread::blocking::AsyncOp;


fn main() {
    // Create an asynchronous operation
    let op = AsyncOp::new(status::PENDING);

    // Split it into a client and a server
    let (op_server, mut op_client) = op.split();

    // Check initial status
    println!("Initial operation status is {:?}", op_client.status());
    {
        // Update the status, then drop the server
        let mut server = op_server;
        server.update(status::RUNNING);
        println!("New operation status is {:?}", op_client.status());
    }

    // Check final status
    println!("Final operation status is {:?}", op_client.status());
    assert_eq!(op_client.status(), status::ERROR_SERVER_KILLED);
}
