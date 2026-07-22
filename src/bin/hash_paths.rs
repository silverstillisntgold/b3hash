use b3hash::{DirectoryHasher, HashingError, Manifest, verify};
use std::{env, time};

fn main() -> Result<(), HashingError> {
    println!("starting program");
    match env::args().len() {
        1 => println!("need a path to hash"),
        2 => {
            let _ = single_path(env::args().nth(1).unwrap())?;
        }
        3 => double_path(env::args().nth(1).unwrap(), env::args().nth(2).unwrap())?,
        _ => println!("too many arguments"),
    }
    println!("program completed");
    Ok(())
}

fn single_path(path: String) -> Result<Manifest, HashingError> {
    let hasher = DirectoryHasher::builder()
        .directory_path(path.into())
        .build();
    let start = time::Instant::now();
    let res = hasher.hash()?;
    let delta = start.elapsed().as_secs_f64();
    println!("hashing successful, completed in:");
    println!("\t{:.2} seconds", delta);
    println!("\t{:.2} ms", delta * 1e3);
    println!("directory path: {:?}", res.path());
    println!("directory name: {}", res.name());
    println!("directory hash: {}", res.hash());
    println!("directory size: {} bytes", res.size());
    println!("entries hashed: {}", res.entries().len());
    Ok(res)
}

fn double_path(path_1: String, path_2: String) -> Result<(), HashingError> {
    let m1 = single_path(path_1)?;
    let m2 = single_path(path_2)?;
    match verify(m1, m2) {
        None => println!("no difference found"),
        Some(diff) => {
            println!("difference found:");
            println!("{:#?}", diff)
        }
    }
    Ok(())
}
