# Rian

**This codebase is intended only as a demonstration of some ideas, not for real-world use.**

A prototypical Rust framework for low-latency event processing built with actors as the primitive.

The intention is for an event processing system to be built on many actors, each of which can be swapped out for testing.

There are several fun ideas incorporated here:

* A dynamic loading system built on the nightly ptr_metadata feature that allows for actors to be swapped out without recompilation
* A runtime that encourages passing messages between actors on the same thread, avoiding slow cross-thread communication
* Upfront allocation of actors on a thread into a contiguous memory region when the process starts
* Memory layout tricks with the ptr_metadata feature; actors of the same type can be packed together for more locality when using dynamic dispatch
* Abstracting message passing between actors; a message is passed between actors on the same thread via a direct function call or passed to another thread via a queue transparently
* Upfront detection of cycles such that an actor can mutably access another actor on the same thread without any runtime checks

I have not written documentation for this project and I don't intend to build on it any further since I have no immediate use for it, but I think it's interesting enough to release as a proof of concept.