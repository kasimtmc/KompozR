//! The Linux event loop and what it is made of: wakes from other threads, the deadline and the key-repeat timers,
//! watched file descriptors, SIGTERM as a quit; memfds and mappings, pipes. One thread, in sequence (signals reach
//! the process).

#[cfg(target_os = "linux")]
mod linux {
    use kompoz_platform::linux::event_loop::EventLoop;
    use kompoz_platform::linux::sys::{self, Mmap};
    use std::io::{Read, Write};
    use std::os::fd::{AsFd, AsRawFd};
    use std::time::{Duration, Instant};

    pub fn a_wake_from_another_thread_ends_the_wait() {
        let mut lp = EventLoop::new().unwrap();
        let waker = lp.waker();
        let start = Instant::now();
        let t = std::thread::spawn(move || {
            std::thread::sleep(Duration::from_millis(30));
            waker.wake();
            waker.wake(); // (twice: one wake)
        });
        let ready = lp.wait(Some(Duration::from_secs(5))).unwrap();
        t.join().unwrap();
        assert!(ready.woken && !ready.deadline && ready.repeats == 0 && ready.sources.is_empty());
        assert!(start.elapsed() >= Duration::from_millis(25) && start.elapsed() < Duration::from_secs(4));
        // Taken: nothing more is ready.
        let ready = lp.wait(Some(Duration::ZERO)).unwrap();
        assert!(!ready.any());
        // The host's waker is the same.
        lp.waker().as_fn()();
        assert!(lp.wait(Some(Duration::ZERO)).unwrap().woken);
    }

    pub fn the_deadline_wakes_at_its_time_once() {
        let mut lp = EventLoop::new().unwrap();
        let at = sys::monotonic_now() + 40_000_000;
        lp.set_deadline(Some(at));
        let ready = lp.wait(Some(Duration::from_secs(5))).unwrap();
        assert!(ready.deadline && !ready.woken);
        assert!(sys::monotonic_now() >= at);
        // Disarmed once it came; a deadline set aside doesn't come.
        assert!(!lp.wait(Some(Duration::from_millis(60))).unwrap().any());
        lp.set_deadline(Some(sys::monotonic_now() + 20_000_000));
        lp.set_deadline(None);
        assert!(!lp.wait(Some(Duration::from_millis(60))).unwrap().any());
        // One past comes at once.
        lp.set_deadline(Some(sys::monotonic_now() - 1_000_000));
        assert!(lp.wait(Some(Duration::from_secs(1))).unwrap().deadline);
    }

    pub fn a_held_key_repeats_until_stopped() {
        let mut lp = EventLoop::new().unwrap();
        lp.start_repeat(Duration::from_millis(30), Duration::from_millis(10));
        let start = Instant::now();
        let first = lp.wait(Some(Duration::from_secs(5))).unwrap();
        assert!(first.repeats >= 1 && start.elapsed() >= Duration::from_millis(25));
        std::thread::sleep(Duration::from_millis(55));
        let late = lp.wait(Some(Duration::ZERO)).unwrap();
        assert!(late.repeats >= 4, "{late:?}"); // (the loop was late: each repeat due counted)
        lp.stop_repeat();
        std::thread::sleep(Duration::from_millis(30));
        assert_eq!(lp.wait(Some(Duration::ZERO)).unwrap().repeats, 0);
    }

    pub fn watched_fds_say_when_they_can_be_read() {
        let mut lp = EventLoop::new().unwrap();
        let (read, write) = sys::pipe(true).unwrap();
        lp.watch(read.as_raw_fd(), 7);
        assert!(!lp.wait(Some(Duration::ZERO)).unwrap().source(7));
        let mut w = std::fs::File::from(write);
        w.write_all(b"merhaba").unwrap();
        let ready = lp.wait(Some(Duration::from_secs(1))).unwrap();
        assert!(ready.source(7) && ready.sources == [7]);
        let mut r = std::fs::File::from(read);
        let mut text = [0u8; 7];
        r.read_exact(&mut text).unwrap();
        assert_eq!(&text, b"merhaba");
        // Non-blocking: nothing more to read now.
        assert_eq!(r.read(&mut text).map_err(|e| e.kind()), Err(std::io::ErrorKind::WouldBlock));
        lp.unwatch(7);
        w.write_all(b"x").unwrap();
        assert!(!lp.wait(Some(Duration::ZERO)).unwrap().source(7));
    }

    pub fn sigterm_is_a_quit_that_wakes_the_loop() {
        let mut lp = EventLoop::new().unwrap();
        lp.quit_on_signals().unwrap();
        assert!(!lp.take_quit_signal());
        sys::raise(libc::SIGTERM).unwrap();
        let ready = lp.wait(Some(Duration::from_secs(5))).unwrap();
        assert!(ready.woken);
        assert!(lp.take_quit_signal());
        assert!(!lp.take_quit_signal()); // (taken)
        sys::raise(libc::SIGINT).unwrap();
        assert!(lp.wait(Some(Duration::from_secs(5))).unwrap().woken && lp.take_quit_signal());
    }

    pub fn memfds_map_shared_and_keymaps_privately() {
        let fd = sys::memfd("kompoz-test").unwrap();
        let file = std::fs::File::from(fd.try_clone().unwrap());
        file.set_len(4096).unwrap();
        let mut a = Mmap::shared(fd.as_fd(), 4096, true).unwrap();
        a.as_mut_slice()[..4].copy_from_slice(b"KZ01");
        let b = Mmap::shared(fd.as_fd(), 4096, false).unwrap();
        assert_eq!(&b.as_slice()[..4], b"KZ01"); // (one memory)
        let keymap = Mmap::private_read(fd.as_fd(), 4096).unwrap();
        assert_eq!(keymap.len(), 4096);
        assert_eq!(&keymap.as_slice()[..4], b"KZ01");
        assert!(Mmap::shared(fd.as_fd(), 0, false).is_err());
    }

    pub fn eventfds_and_timerfds_count() {
        let e = sys::eventfd().unwrap();
        assert_eq!(sys::eventfd_read(e.as_fd()).unwrap(), 0);
        sys::eventfd_write(e.as_fd(), 2).unwrap();
        sys::eventfd_write(e.as_fd(), 3).unwrap();
        assert_eq!(sys::eventfd_read(e.as_fd()).unwrap(), 5);
        let t = sys::timerfd().unwrap();
        assert_eq!(sys::timerfd_read(t.as_fd()).unwrap(), 0);
        sys::timerfd_set_repeating(t.as_fd(), Duration::from_millis(1), Duration::from_millis(1)).unwrap();
        std::thread::sleep(Duration::from_millis(10));
        assert!(sys::timerfd_read(t.as_fd()).unwrap() >= 5);
    }
}

fn main() {
    #[cfg(target_os = "linux")]
    {
        let tests: &[(&str, fn())] = &[
            ("a_wake_from_another_thread_ends_the_wait", linux::a_wake_from_another_thread_ends_the_wait),
            ("the_deadline_wakes_at_its_time_once", linux::the_deadline_wakes_at_its_time_once),
            ("a_held_key_repeats_until_stopped", linux::a_held_key_repeats_until_stopped),
            ("watched_fds_say_when_they_can_be_read", linux::watched_fds_say_when_they_can_be_read),
            ("sigterm_is_a_quit_that_wakes_the_loop", linux::sigterm_is_a_quit_that_wakes_the_loop),
            ("memfds_map_shared_and_keymaps_privately", linux::memfds_map_shared_and_keymaps_privately),
            ("eventfds_and_timerfds_count", linux::eventfds_and_timerfds_count),
        ];
        let filters: Vec<String> = std::env::args().skip(1).filter(|a| !a.starts_with('-')).collect();
        let mut failed = 0;
        for (name, test) in tests {
            if !filters.is_empty() && !filters.iter().any(|f| name.contains(f.as_str())) {
                continue;
            }
            let ok = std::panic::catch_unwind(test).is_ok();
            failed += usize::from(!ok);
            println!("{} {name}", if ok { "ok    " } else { "FAILED" });
        }
        if failed > 0 {
            std::process::exit(1);
        }
    }
}
