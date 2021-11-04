use std::time::Duration;
use closure_future::closure_future;

fn main() {
    let mut futures = Vec::new();
    for i in 0..16 {
        let (future, worker) = closure_future(move || {
            let wait_time = i * 1337 % 17;
            println!("Beg {} (wait {} sec)", i, wait_time);
            std::thread::sleep(Duration::from_secs(i * 1337 % 17));
            println!("End {}", i);
            i
        });
        rayon::spawn(|| worker.run());
        futures.push(future)
    }
    for fut in futures {
        println!("{:?}", smol::block_on(fut));
    }
}
