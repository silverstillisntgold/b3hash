const MEBIBYTE: f64 = (1 << 20) as f64;

fn main() -> std::io::Result<()> {
    println!("running program");
    let path = std::env::args()
        .nth(1)
        .expect("please enter directory path");

    //let (r, t) = time(|| b3hash::fs::get_files(&path));
    //let r = r?;
    //println!("time: {:.2} seconds || len: {}", t, r.len());

    //let (res, t) = time(|| b3hash::create_hashfile(&path));
    //let _ = res?;
    //println!("Execution time: {:.2}", t);

    /*let (res, t) = time(|| b3hash::validate_hashfile(&path));
    let res = res?;
    if res.is_none() {
        println!("all files validated");
        println!("time: {:.2}", t);
    } else {
        println!("validation failed:");
        println!("files failes: {}", res.unwrap().len());
    }*/

    let (res, t) = time(|| b3hash::hash_directory(&path));
    let res = res?;
    println!("Execution time: {:.2} seconds", t);
    println!("Directory name: {}", res.name);
    println!("Directory checksum: {}", res.hash.to_hex());
    println!("File count: {}", res.len());
    println!("Final size in bytes: {}", res.size);
    println!("Final size in megabytes: {:.2}", res.size as f64 / 1e6);
    println!("Final size in gigabytes: {:.2}", res.size as f64 / 1e9);
    println!("Throughput (files/sec): {:.2}", res.len() as f64 / t);
    println!(
        "Execution speed: {:.2} MiB/s",
        res.size as f64 / t / MEBIBYTE
    );

    println!();
    Ok(())
}

fn time<F, R>(func: F) -> (R, f64)
where
    F: FnOnce() -> R,
{
    let start = std::time::Instant::now();
    let res = func();
    let delta = std::time::Instant::now()
        .duration_since(start)
        .as_secs_f64();
    (res, delta)
}
