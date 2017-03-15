# An asynchronous operation monitoring experiment

## Motivation

In a world of massively parallel computer hardware, delegating work has become a
performance-critical software engineering skill.

It is now widely acknowledged that offloading as much work as possible to other
CPU cores, DMA-enabled IO interfaces, specialized co-processors such as GPUs, or
even remote servers over a network, is often critical to optimal performance on
modern hardware. It is also acknowledged that waiting for such operations to
complete when there is more work to do is a waste of valuable CPU time, and that
spawning lots of threads to handle such blocking workloads is a waste of system
resources. For all of these reasons, the future of high-performance applications
lies in extensive use of asynchronous APIs, in which a client is not forced to
wait for the completion of an offloaded operation before doing something else.

At the heart of any asynchronous API, asynchronous requests are modeled as state
machines, whose states can be roughly classified in three broad categories:

- Before an asynchronous request is validated and begins execution (pending)
- During an asynchronous request's execution, if accepted by a server (running)
- After the server is done with the request (done, erronerous, or cancelled)

The purpose of an asynchronous API is to give a client access to this state
machine and means to monitor its evolution. All APIs do so in some of the
following ways:

- Let a client query the status of the asynchronous operation (polling)
- Allow a client to wait for status changes (blocking)
- Schedule client-specified code to run when the status changes (callbacks)

The takeaway point here is that at a conceptual level, all asynchronous APIs
follow very similar design principles. Unfortunately, the same cannot be said
of their implementation. Far from the conceptual homogeneity of blocking APIs,
asynchronous APIs are usually implemented using a wild mixture of incompatible
software abstractions. Examples include:

- Futures and message streams, in eager ("push") and lazy ("pull") flavours
- Subtly incompatible variants of the command queue concept
- OpenCL-like event objects
- POSIX 1b asynchronous IO requests
- poll(), epoll(), select(), kqueue(), and API-specific variants thereof
- Process-local callbacks and UNIX signals

In this context, this project aims as answering the following questions:

1. Could this set of incompatible asynchronous programming abstractions be
   simplified back into a consistent design where each request is treated as a
   state machine, with the state synchronization methods described above?
2. Could state machines span the entire range of client/server communication
   perimeters on which asynchronous abstractions are used today, from coroutines
   to networked requests with thread synchronization and IPC inbetween?
3. Would such an approach be sufficiently efficient for people to be
   realistically interested in using it in practice?

If this project is successful, the expected benefit is to open the way for a
consistent and efficient asynchronous programming model, usable at any scale,
allowing for abstractions which are more familiar to programmers rather than
being domain-specific, and enabling fearless major implementation changes
without interface breakages for API designers.


## Current status

This project is highly experimental in nature, and the current implementation
is very preliminary. Currently, we have initial work on asynchronous operation
state machines where the client and the server are two different threads
working in the same process, where monitoring is done at the granularity of
individual operations.

Future areas to be explored include:

- Monitoring multiple operations concurrently using an ANY/ALL operator
- Expanding the abstraction to smaller scales (coroutines) and larger scales
  (IPC, OpenCL & IO, RPC)
- Extra options for asynchronous callback scheduling
