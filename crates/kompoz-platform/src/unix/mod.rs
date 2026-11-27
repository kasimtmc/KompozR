//! What KompozR's Linux and Android platforms share of the Linux kernel and its C libraries (glibc's, bionic's):
//! eventfds, timerfds, `poll`, the monotonic clock, pipes, `dlopen` ([`sys`]), and the waker an eventfd makes
//! ([`waker`]). Linux's own (memfds, inotify, V4L2, signals) and its event loop are in `linux`.

pub mod sys;
pub mod waker;
