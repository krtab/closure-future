use closure_future::closure_future;
use std::time::Duration;

fn main() {
    let mut futures = Vec::new();
    for i in 0..16 {
        let (future, worker) = closure_future(move || {
            println!("Beg {}", i);
            std::thread::sleep(Duration::from_secs(1));
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
