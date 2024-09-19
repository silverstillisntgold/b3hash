fn main() -> std::io::Result<()> {
    const MEG: f64 = (1 << 20) as f64;

    let path = std::env::args()
        .nth(1)
        .expect("please enter directory path");

    /*let (res, t) = time(|| b3hash::create_hashfile(&path));
    let _ = res?;
    println!("Execution time: {:.2}", t);
    //return Ok(());

    let (res, t) = time(|| b3hash::validate_hashfile(&path));
    let res = res?;
    if res.is_none() {
        println!("all files validated");
        println!("time: {:.2}", t);
    } else {
        println!("validation failed:");
        println!("{:?}", res.unwrap());
    }
    println!();
    return Ok(());*/

    let (res, time) = time(|| b3hash::hash_directory(&path));
    let res = res?;
    println!("Execution time: {:.2} seconds", time);
    println!("Directory name: {}", res.dir_name);
    println!("Directory checksum: {}", res.hash.to_hex());
    println!("File count: {}", res.len());
    println!("Final size in bytes: {}", res.size);
    println!("Final size in megabytes: {:.2}", res.size as f64 / 1e6);
    println!("Final size in gigabytes: {:.2}", res.size as f64 / 1e9);
    println!(
        "Execution speed: {:.2} MiB/s",
        (res.size) as f64 / time / MEG
    );
    println!();

    Ok(())
}

#[inline(always)]
fn time<F, R>(func: F) -> (R, f64)
where
    F: FnOnce() -> R,
{
    let start = std::time::Instant::now();
    let res = func();
    let time_delta = std::time::Instant::now()
        .duration_since(start)
        .as_secs_f64();
    (res, time_delta)
}
