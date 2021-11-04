This crate makes it easy to transform functions or closures into two objects: one that can be run on a thread pool, and a [`Future`] on its result. Contrarily to other existing solution, starting the computation is independent from polling the future. It is entirely independent of both the thread or thread pool used to run the actual computations and the future executor.

The entry-point is [`closure_future`].

Examples
-------

Workers can be run on threads:

```rust
# use std::time::Duration;
# use closure_future::closure_future;
# use pollster::block_on;
let (future, worker) = closure_future(|| {
    // ... do some work and return a value ...
    "Hello!"
});
std::thread::spawn(|| worker.run());
assert_eq!(block_on(future), Ok("Hello!"))
```

Workers can also be run using rayon global thread-pool,
using a provided helper function:

```rust
# use std::time::Duration;
# use closure_future::spawn_rayon;
# use pollster::block_on;
let mut futures = Vec::new();
for i in 0..10 {
    let future = spawn_rayon(move || {
        // ... do some work and return a value ...
        i
    });
    futures.push(future)
}
for (i,fut) in futures.into_iter().enumerate() {
    assert_eq!(block_on(fut),Ok(i));
}
```